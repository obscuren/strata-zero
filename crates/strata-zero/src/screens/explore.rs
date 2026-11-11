//! The map screen: free-camera Explore mode and the Walk mode with the `@`.

use crate::actions::{perform, report};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::interact::{self, FREE_LETTERS};
use crate::messages::Kind;
use crate::outside::stream::{Beyond, GroundMap, Owner};
use crate::render::atrium::AtriumOverlay;
use crate::render::fx::{FxOverlay, ScorchOverlay};
use crate::render::items::GroundOverlay;
use crate::render::map::{MapRef, TileMap};
use crate::render::overlay::Digital;
use crate::render::people::{InteriorPeople, PeopleOverlay};
use crate::render::viewport::{Camera, MapView, Overlay, TilePass, tile_to_screen};
use crate::render::weather::{Fall, Weather};
use crate::screens::configure::Configure;
use crate::screens::console;
use crate::screens::deck::Deck;
use crate::screens::help;
use crate::screens::messages::Messages;
use crate::screens::pause::Pause;
use crate::screens::popup::{Choice, Pick, Popup};
use crate::screens::title::Title;
use crate::ui::chrome::{CYAN, MAGENTA, put};
use crate::ui::console::{Console, ConsoleEvent};
use crate::ui::hud;
use crate::ui::sidebar::{self, Section};
use crate::walker::Player;
use citygen::rng::hash64;
use citygen::{Dir, Place, Pos, Rect as TileRect, StratumKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Widget};
use std::cell::{Cell, RefCell};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Explore,
    Walk,
}

/// Frames per second over the last second.
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

/// Space with several things in reach: a direction opens one.
#[derive(Clone, Debug)]
pub struct WhichWay {
    pub subjects: Vec<interact::Subject>,
    /// Where the player stood when it was asked; a move drops it.
    pub from: (Place, Pos),
}

/// Something the fire cursor can point at: a foe on foot, a piece of ICE in a
/// construct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enemy {
    Foe(usize),
    Ice(usize),
}

/// One entry of the act half's `ACT` list: an ability to fire or a throwable to arm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Ability(game::AbilityId),
    Throw(game::ItemId),
}

/// Look mode: a free cursor on the map the player walks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Looking {
    pub cursor: Pos,
    /// The map it looks at; carried elsewhere, the look ends.
    pub place: Place,
}

/// The arrow a direction reads as in the which-way prompt.
fn arrow(d: Dir) -> char {
    match d {
        Dir::N => '↑',
        Dir::NE => '↗',
        Dir::E => '→',
        Dir::SE => '↘',
        Dir::S => '↓',
        Dir::SW => '↙',
        Dir::W => '←',
        Dir::NW => '↖',
    }
}

pub struct MapScreen {
    pub mode: Mode,
    /// The map on screen: a stratum, or the floor the player walks.
    pub place: Place,
    pub cam: Camera,
    /// The tile under inspection in Explore mode; the camera follows it.
    pub cursor: Pos,
    /// The tile a walk is aimed at, on turns or in a construct.
    pub aim: Option<Pos>,
    /// The fire cursor on your act half: what a shot goes at.
    pub target: Option<Pos>,
    /// A throwable the cursor is aimed with: space throws it at the cursor.
    pub armed: Option<game::ItemId>,
    /// The enemy you aimed at last: the cursor comes back to it while it stands.
    last_enemy: Option<Enemy>,
    /// The half of your turn the last frame saw, so the act half's opening is one
    /// moment: the cursor opens on the nearest foe then.
    last_phase: Option<crate::net::Phase>,
    /// The aimed walk being stepped, a tile per `WALK_TICKS`; keys wait for it.
    pub walking: Option<Walk>,
    /// A ride along a lane of the open Net, a tile per `RIDE_TICKS`; any key stops it.
    pub riding: Option<crate::ride::Ride>,
    /// A program waiting for its target.
    pub targeting: Option<Targeting>,
    /// Space asked which way.
    pub which_way: Option<WhichWay>,
    /// Look mode's cursor.
    pub looking: Option<Looking>,
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
    /// The cord's walk, kept while neither end nor the view moves.
    cord_walk: RefCell<crate::net::CordWalk>,
    /// Where your Net self stood as the Net over the world opened over the map: back
    /// from it somewhere else, the map goes with you.
    up_from: Option<Pos>,
    /// The Nets' traffic: packets hopping node to node along the lanes, each city's
    /// and the open Net's.
    traffic: RefCell<crate::render::netlook::packets::Nets>,
    /// Where the occupants of the floor the player walks stand, keyed by the place
    /// and the world's minute it was worked out for.
    placement: Option<(Place, u64, crate::interiors::Placement)>,
    /// The terminals and gate switches of the site's floor on screen, kept while it stays
    /// on screen: they come of the site's Net, worked out whole, and a mission's copy
    /// waits to begin or end while you're in its building.
    devices: RefCell<Option<(Place, DeviceTiles)>>,
}

/// A site's floor's terminals and gate switches, by their wall tiles.
type DeviceTiles = Vec<(Pos, citygen::NodeKind)>;

pub(crate) fn dir_for(code: KeyCode) -> Option<Dir> {
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

/// NEAR's places in HERE off the city's cells.
const NEAR_ROWS: usize = 4;

/// Whether `place`'s map carries on past the city's edge: the street and the Net, with a
/// world out there.
fn carries_on(st: &AppState, place: Place) -> bool {
    matches!(
        place,
        Place::Stratum(StratumKind::Street | StratumKind::Net)
    ) && Beyond::of(st).is_some()
}

/// The map at `place` at the hour, carried on past the city's edge by `beyond` where it
/// goes on; a floor not yet generated falls back to the street.
fn walked_map<'a>(
    st: &'a AppState,
    city: &'a citygen::City,
    place: Place,
    beyond: Option<&'a Beyond<'a>>,
) -> MapRef<'a> {
    let map = MapRef::for_place(
        city,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    )
    .unwrap_or_else(|| MapRef::stratum_of(city, StratumKind::Street));
    match beyond {
        Some(b) => map.beyond(b),
        None => map,
    }
}

/// You look round again from where you stand, past the city's edge where the map goes
/// on; what you see out there is kept in the world's record.
fn look(st: &mut AppState) {
    let bonus = st.sight_bonus();
    let Some(mut p) = st.player.take() else {
        return;
    };
    if let Some(city) = st.city.as_ref() {
        let beyond = Beyond::of(st);
        p.update_fov(&walked_map(st, city, p.place, beyond.as_ref()), bonus);
    }
    st.player = Some(p);
    crate::outside::stream::remember_sight(st);
}

/// How far round you sight may reach on your place's map at its brightest: the stream
/// is built that far before you look.
fn sight_reach(st: &AppState, p: &Player) -> i32 {
    (p.radius_at(1.0) + st.sight_bonus().max(0)).max(citygen::lighting::tuning::LIT_RADIUS) + 1
}

/// You look round where you've been put (down from the overworld, in from it): the
/// ground in reach of your sight built first where the street carries on.
pub(crate) fn look_round(st: &mut AppState) {
    if let Some(p) = st.player.as_ref()
        && carries_on(st, p.place)
    {
        let r = sight_reach(st, p);
        let area = TileRect::new(p.pos.x - r, p.pos.y - r, 2 * r + 1, 2 * r + 1);
        crate::outside::stream::tend(st, area);
    }
    look(st);
}

/// Whether a step `dir` from where you stand on a stratum lands on ground you can stand
/// on, past the city's edge where the map goes on.
fn open_step(st: &AppState, dir: Dir) -> bool {
    st.player
        .as_ref()
        .is_some_and(|p| open_at(st, p.place, p.pos.step(dir)))
}

/// Whether `to` on stratum `place` is ground you can stand on, past the city's edge where
/// the map goes on.
fn open_at(st: &AppState, place: Place, to: Pos) -> bool {
    let Some(city) = st.city.as_ref() else {
        return false;
    };
    if place.stratum().is_none() {
        return false;
    }
    let beyond = Beyond::of(st);
    walked_map(st, city, place, beyond.as_ref()).walkable(to)
}

/// The ride a step `dir` from `from` takes over the Net, past the city's edge where it
/// goes on (`ride::ride_path`), and whether it stops only short of the Net not built
/// yet there (`ride::runs_on`).
fn ride_over(st: &AppState, from: Pos, dir: Dir) -> (Vec<Pos>, bool) {
    let Some(city) = st.city.as_ref() else {
        return (Vec::new(), false);
    };
    let beyond = Beyond::of(st);
    let net = walked_map(st, city, Place::Stratum(StratumKind::Net), beyond.as_ref());
    let path = crate::ride::ride_path(&net, from, dir);
    let on = beyond.is_some() && crate::ride::runs_on(&net, from, &path);
    (path, on)
}

/// Why a step from `from` to `to` may not cross now: on foot on the street, off the
/// cells the city owns or into another city's or town's; your Net self on the Net, into
/// another's (a fight, a call, a scene, a run in flight). `None` when it may, or when it
/// doesn't cross. The Net self walks off onto the open Net freely.
fn crossing_refused(st: &AppState, from: Pos, to: Pos) -> Option<&'static str> {
    let p = st.player.as_ref()?;
    let net = p.jacked_in() && p.place == Place::Stratum(StratumKind::Net);
    if !net && (!p.place.is_street() || p.jacked_in()) {
        return None;
    }
    let live = |q: Pos| crate::outside::stream::live_owns(st, q);
    let leaving = !net && live(from) && !live(to);
    // onto another city's or town's cells: walking in, as leaving is
    let into = !live(to) && {
        let owner = |q: Pos| crate::outside::stream::owner_of(st, q);
        let here = st.outside.as_ref().map(|o| o.here());
        owner(to).is_some() && owner(to) != here && owner(to) != owner(from)
    };
    if !leaving && !into {
        return None;
    }
    crate::outside::can_cross(st).err()
}

/// After a step on the street off the cells the city owns, or back onto them: at the
/// MAP's mark, it goes.
fn stepped_outside(st: &mut AppState, from: Pos) {
    let Some(p) = st.player.as_ref() else {
        return;
    };
    let off = |q: Pos| !crate::outside::stream::live_owns(st, q);
    if p.place.is_street() && !p.jacked_in() && (off(from) || off(p.pos)) {
        crate::outside::reached_mark(st);
    }
}

/// Whether a port of the open ground is in reach, off the cells the city owns, on foot:
/// a terminal or a vending machine on your cell or beside it, whoever's ground it is.
fn ground_port_in_reach(st: &AppState) -> bool {
    use citygen::{Feature, Furniture};
    let Some(p) = st.player.as_ref() else {
        return false;
    };
    if !p.place.is_street() || p.jacked_in() || crate::outside::stream::live_owns(st, p.pos) {
        return false;
    }
    let (Some(city), Some(beyond)) = (st.city.as_ref(), Beyond::of(st)) else {
        return false;
    };
    let street = walked_map(st, city, p.place, Some(&beyond));
    std::iter::once(p.pos).chain(p.pos.neighbors8()).any(|q| {
        street.tile(q).is_some_and(|t| {
            matches!(
                t.feature,
                Some(Feature::Terminal) | Some(Feature::Furniture(Furniture::Vending))
            )
        })
    })
}

/// Space by a port of the open ground (`ground_port_in_reach`): jacked in there, your
/// Net self on the nearest cell of the Net with signal (`net::ground_port`), your body
/// left where it stands. Refused, with the word why, as the city's ports refuse.
fn jack_in_on_the_ground(st: &mut AppState) {
    let Some((place, pos)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    if crate::net::engaged_on_foot(st) {
        st.warn("not with a gun on you");
        return;
    }
    if Beyond::of(st).is_none() {
        return;
    }
    if let Some(left) = st.net.cooling(st.net.now(st)) {
        st.warn(format!("deck cooling · {left} min"));
        return;
    }
    let Some(start) = crate::net::ground_port(st, pos) else {
        st.warn("no signal here");
        return;
    };
    if let Some(p) = st.player.as_mut() {
        p.body = Some((place, pos));
        p.place = Place::Stratum(StratumKind::Net);
        p.pos = start;
    }
    look(st);
    crate::net::after_jack_in(st, place, None);
    st.say("jacked in · the open Net");
}

/// Why a step on foot onto `tile` doesn't go, by what's there and nothing else, in the
/// outside screen's words: water, a climb, the world's end (past it, or ground not made).
/// A wall, or anything standing in the way, says nothing, as in the city: the same
/// wherever it is, the live city's, another's or the open ground's.
fn why_not(tile: Option<&citygen::Tile>) -> Option<&'static str> {
    use citygen::Terrain;
    match tile.map(|t| t.terrain) {
        None | Some(Terrain::Void) => Some("the world ends here"),
        Some(Terrain::Water | Terrain::Shallows) => Some("too deep to wade"),
        Some(Terrain::Cliff | Terrain::Rock) => Some("too steep to climb"),
        Some(_) => None,
    }
}

/// A step `dir` on foot on the street that didn't go: what's there says why
/// (`why_not`), whoever's cell it is. On the Net nothing is said.
fn say_why_not(st: &mut AppState, dir: Dir) {
    let Some(p) = st.player.as_ref() else {
        return;
    };
    let to = p.pos.step(dir);
    if !p.place.is_street() || p.jacked_in() {
        return;
    }
    let line = {
        let (Some(city), Some(b)) = (st.city.as_ref(), Beyond::of(st)) else {
            return;
        };
        why_not(walked_map(st, city, p.place, Some(&b)).tile(to))
    };
    if let Some(line) = line {
        st.warn(line);
    }
}

/// Whether a step `dir` on the Net goes onto a gate of another city's or town's Net
/// (found through who owns the cell, on its own map) from across the line: one only its
/// own cells, where it's the live city, go through.
fn foreign_gate(st: &AppState, dir: Dir) -> bool {
    let Some(p) = st.player.as_ref() else {
        return false;
    };
    if p.place != Place::Stratum(StratumKind::Net) {
        return false;
    }
    let Some(b) = Beyond::of(st) else {
        return false;
    };
    matches!(
        b.elsewhere(p.pos.step(dir)),
        Some(Owner::City(city, q)) if city
            .tile(StratumKind::Net, q)
            .is_some_and(|t| t.terrain == citygen::Terrain::NetGate)
    )
}

/// One step `dir` over the map you stand on (past the city's edge where it goes on), and
/// a look round after it. Whether you moved.
fn step(st: &mut AppState, dir: Dir) -> bool {
    let Some(mut p) = st.player.take() else {
        return false;
    };
    let from = p.pos;
    let moved = st.city.as_ref().is_some_and(|city| {
        let beyond = Beyond::of(st);
        p.try_move(&walked_map(st, city, p.place, beyond.as_ref()), dir)
    });
    st.player = Some(p);
    if moved {
        look(st);
        stepped_outside(st, from);
    }
    moved
}

/// What the street or the Net shows past the city's edge, drawn before the city draws
/// on the cells it owns: the open ground's layer on every cell nobody held answers for,
/// then each other city and town held this session on the cells it owns, each with its
/// own map and its own Net. Alive as the city is: on the street each city's people
/// (theirs in sight as yours are), on the Net each city's packets on its own lanes and
/// the open Net's on its trunks, all moved on in `nets`. `vis` says how well a cell of
/// the city's frame is seen.
#[allow(clippy::too_many_arguments)]
fn draw_beyond(
    st: &AppState,
    b: &Beyond,
    stratum: StratumKind,
    origin: Pos,
    area: Rect,
    vis: &dyn Fn(Pos) -> crate::render::Visibility,
    nets: &mut crate::render::netlook::packets::Nets,
    buf: &mut ratatui::buffer::Buffer,
) {
    use crate::render::netlook::packets::{Lit, Traffic};
    use crate::render::netlook::{NetArea, NetLookView, draw_area};
    use crate::render::people::StreetPeople;
    let minute = st.minute_of_day();
    let net = stratum == StratumKind::Net;
    let (hour, _) = st.clock();
    let night = hour >= 18 || hour <= 5;
    // the view in the live city's frame
    let view = TileRect::new(origin.x, origin.y, area.width as i32, area.height as i32);
    // each area in its own frame: its cell `q` is the city's `q - shift`; on the
    // street its people over its ground, on the Net its packets on its lanes
    let draw = |map: &dyn TileMap,
                city: Option<&citygen::City>,
                people: Option<&sim::World>,
                lit: Lit,
                shift: Pos,
                keep: &dyn Fn(Pos) -> bool,
                buf: &mut ratatui::buffer::Buffer| {
        let corner = origin.offset(shift.x, shift.y);
        let seen = |q: Pos| vis(Pos::new(q.x - shift.x, q.y - shift.y));
        if net {
            let area_net = NetArea {
                map,
                city,
                keep: Some(keep),
            };
            // the Net out here: nothing tracked, nobody's home
            let (packets, soaks) = lit;
            let look = NetLookView {
                tick: st.tick,
                packets,
                soaks,
                ..Default::default()
            };
            draw_area(area_net, &look, corner, area, buf, &st.style);
        } else {
            TilePass {
                map,
                stratum,
                origin: corner,
                tick: st.tick,
                style: &st.style,
                vis: &seen,
                keep: Some(keep),
            }
            .draw(area, buf);
        }
        // the same rain falls on both sides of the edge
        if st.rain {
            let fall = Fall {
                map,
                stratum,
                keep: Some(keep),
            };
            fall.draw(corner, area, st.tick, buf, &st.style);
        }
        // its people out on its street, in sight as the city's own are
        if let Some(world) = people.filter(|_| !net) {
            StreetPeople {
                world,
                night,
                seen: &seen,
                keep: Some(keep),
                eyes: &[],
            }
            .draw(corner, area, buf, &st.style);
        }
        // and the dev light view greys every area by its own light, as the city's
        if st.show_light {
            let seen = |q: Pos| match net {
                true => crate::render::Visibility::Visible,
                false => seen(q),
            };
            crate::render::lighting::LightPass {
                map,
                vis: &seen,
                keep: Some(keep),
            }
            .draw(corner, area, buf);
        }
    };
    // the open ground, and every settlement not held, in world cells: on the Net its
    // trunks' packets, on the cells it draws
    let live = b.origin();
    let ground = GroundMap {
        stream: b.stream(),
        layer: stratum,
        minute,
    };
    let open = |w: Pos| {
        matches!(
            b.elsewhere(Pos::new(w.x - live.x, w.y - live.y)),
            Some(Owner::Ground(..))
        )
    };
    let lit = match st.outside.as_ref().and_then(|o| o.world.as_ref()) {
        Some(world) if net => {
            let layer = &b.stream().net;
            let tile = |w: Pos| layer.tile(w).filter(|_| open(w));
            let at = TileRect::new(view.x + live.x, view.y + live.y, view.w, view.h);
            let near = layer
                .cells_in(Traffic::seeds(at))
                .filter(|(_, t)| {
                    t.terrain == citygen::Terrain::NetLane
                        || matches!(t.feature, Some(citygen::Feature::Node(_)))
                })
                .map(|(w, _)| w);
            nets.open(world.seed).run_sparse(
                |w| tile(w).is_some_and(|t| t.terrain == citygen::Terrain::NetLane),
                |w| tile(w).is_some_and(|t| matches!(t.feature, Some(citygen::Feature::Node(_)))),
                near,
                at,
                st.tick,
            )
        }
        _ => Default::default(),
    };
    draw(&ground, None, None, lit, live, &open, buf);
    // the other cities and towns held, each on the cells it owns
    for held in b.held() {
        let (city, at) = (held.city, held.origin);
        let shift = Pos::new(live.x - at.x, live.y - at.y);
        let theirs = TileRect::new(view.x + shift.x, view.y + shift.y, view.w, view.h);
        if !theirs.intersects(city.size.rect()) {
            continue;
        }
        let own = |q: Pos| {
            matches!(
                b.elsewhere(Pos::new(q.x - shift.x, q.y - shift.y)),
                Some(Owner::City(c, _)) if std::ptr::eq(c, city)
            )
        };
        let lit = match net {
            true => nets.city(city.seed).run_city(city, theirs, st.tick),
            false => Default::default(),
        };
        let map = MapRef::stratum_at(city, stratum, minute);
        draw(&map, Some(city), held.people, lit, shift, &own, buf);
    }
}

/// The holo ads hijacked in the cities and towns held as you left them, on the cells
/// each owns that `vis` says are in sight: cells of the live city's frame.
fn hijacked_beyond(
    st: &AppState,
    b: &Beyond,
    vis: &dyn Fn(Pos) -> crate::render::Visibility,
) -> Vec<Pos> {
    let Some(o) = st.outside.as_ref() else {
        return Vec::new();
    };
    let (now, live) = (st.net.now(st), b.origin());
    crate::outside::held_stays(st)
        .flat_map(|stay| {
            let at = o.origin(&stay.city);
            let shift = Pos::new(at.x - live.x, at.y - live.y);
            crate::holds::hijacked_ads_in(&stay.city, &stay.net, now)
                .into_iter()
                .map(move |(q, _)| (stay, q.offset(shift.x, shift.y)))
        })
        .filter(|(stay, p)| {
            matches!(b.elsewhere(*p), Some(Owner::City(c, _)) if std::ptr::eq(c, &stay.city))
                && vis(*p) == crate::render::Visibility::Visible
        })
        .map(|(_, p)| p)
        .collect()
}

/// The footer's slots on foot: your abilities, cooling or ready, lit a moment after firing.
pub(crate) fn ability_slots(st: &AppState) -> Vec<hud::Slot> {
    MapScreen::actives(st)
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

/// The grid: a four-row band, a heavy rule, the map beside the right column, a heavy
/// rule, then the two footer rows. One light vertical rule separates the columns.
pub(crate) struct Regions {
    pub(crate) band_l: [Rect; 4],
    pub(crate) band_r: [Rect; 4],
    pub(crate) top_rule: Rect,
    /// The route and the trace home, under the rule; only when there is something to say.
    pub(crate) route: Option<Rect>,
    pub(crate) map: Rect,
    pub(crate) side: Rect,
    pub(crate) vrule_x: u16,
    pub(crate) bottom_rule: Rect,
    pub(crate) footer: [Rect; 4],
}

pub(crate) fn layout(area: Rect, show_route: bool) -> Regions {
    let r = sidebar::width(area.width);
    let l = area.width.saturating_sub(r + 1);
    let x_rule = area.x + l;
    let row = |y: u16, x: u16, w: u16| Rect::new(x, area.y + y, w, 1);
    let main_y = area.y + 5;
    let main_h = area.height.saturating_sub(10);
    let give = u16::from(show_route && main_h > 0);
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
        route: (give == 1).then(|| row(5, area.x, l)),
        map: Rect::new(area.x, main_y + give, l, main_h - give),
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

    /// The map on foot over a run just opened, you where you stood: its first frame runs
    /// the world a frame on from the saved tick, not the minute a gap since an update
    /// would.
    pub fn resumed(st: &mut AppState) -> Self {
        let mut s = Self::walking(st);
        s.last_tick = st.tick;
        s
    }

    /// A film scene's frame over the map drawn into `map_area`: the doors and the marker,
    /// then everything but the map in black with the subtitle, a keypad over it, and the
    /// console and help if open.
    fn draw_film(&self, st: &AppState, f: &mut Frame, map_area: Rect, origin: Pos) {
        use crate::missions::{film, overlay, scene};
        let area = f.area();
        overlay::draw(f.buffer_mut(), map_area, origin, st);
        film::draw(f.buffer_mut(), area, map_area, st);
        if let Some(pad) = scene::running(st).and_then(|s| s.pad()) {
            crate::ui::dim::wash_rect(f.buffer_mut(), map_area);
            pad.draw(st, f);
        }
        if let Some(con) = self.console.as_ref() {
            let row = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
            f.render_widget(
                Paragraph::new(console::line(con, st.tick, st.log.recent(st.tick))),
                row,
            );
        }
        if self.help {
            help::draw(f, self.mode, st.dev_unlocked);
        }
    }

    /// The map's rect as last drawn.
    #[cfg(test)]
    pub(crate) fn map_area(&self) -> Rect {
        self.last_map.get()
    }

    /// Drawn once at `w`×`h`, then a few frames run with no keys, as play has it before
    /// anything is looked at: the camera on you and clamped to the map it shows.
    #[cfg(test)]
    pub(crate) fn settle(&mut self, st: &mut AppState, (w, h): (u16, u16)) {
        let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        t.draw(|f| self.draw(st, f)).unwrap();
        for _ in 0..20 {
            self.update(st, &[]);
        }
    }

    /// The map area of the last frame, for a shot that checks where something drew.
    #[cfg(test)]
    pub(crate) fn last_map_rect(&self) -> Rect {
        self.last_map.get()
    }

    /// The tile this map's placement stands `id` on, on the floor the player walks.
    #[cfg(test)]
    pub(crate) fn placed_tile(&self, id: sim::PersonId) -> Option<Pos> {
        let (_, _, placement) = self.placement.as_ref()?;
        placement
            .people()
            .find(|(_, who)| *who == id)
            .map(|(p, _)| p)
    }

    /// Look mode is open.
    #[cfg(test)]
    pub(crate) fn is_looking(&self) -> bool {
        self.looking.is_some()
    }

    /// On foot with nothing of the map's own open: no console, look, pick, aim, armed
    /// shot, walk in flight or help. Only then are space and esc a HOLO call's.
    fn plain_walk(&self) -> bool {
        self.mode == Mode::Walk
            && self.console.is_none()
            && self.looking.is_none()
            && self.which_way.is_none()
            && self.targeting.is_none()
            && self.aim.is_none()
            && self.armed.is_none()
            && self.walking.is_none()
            && !self.help
    }

    pub fn explore(_tick: u64) -> Self {
        MapScreen {
            mode: Mode::Explore,
            place: Place::Stratum(StratumKind::Street),
            cam: Camera::default(),
            cursor: Pos::new(0, 0),
            aim: None,
            target: None,
            armed: None,
            last_enemy: None,
            last_phase: None,
            walking: None,
            riding: None,
            targeting: None,
            which_way: None,
            looking: None,
            digital: false,
            help: false,
            fps: FpsMeter::default(),
            console: None,
            last_tick: 0,
            program_flash: None,
            cam_ready: false,
            last_map: Cell::new(Rect::new(0, 0, 100, 28)),
            cord_walk: RefCell::default(),
            up_from: None,
            traffic: RefCell::new(Default::default()),
            placement: None,
            devices: RefCell::new(None),
        }
    }

    /// The terminals and gate switches of the floor on screen (`sitenet::floor_devices`),
    /// none off a floor; worked out once while the floor stays on screen.
    fn floor_devices(&self, st: &AppState) -> DeviceTiles {
        let mut kept = self.devices.borrow_mut();
        let Place::Floor { building, level } = self.place else {
            *kept = None;
            return Vec::new();
        };
        if let Some((place, devices)) = kept.as_ref()
            && *place == self.place
        {
            return devices.clone();
        }
        let devices = crate::sitenet::floor_devices(st, building, level);
        *kept = Some((self.place, devices.clone()));
        devices
    }

    /// Your act half on turns, on any map: the keys aim and fire.
    fn firing(&self, st: &AppState) -> bool {
        self.mode == Mode::Walk
            && st.net.engaged.is_some()
            && crate::net::phase(st) == Some(crate::net::Phase::Act)
    }

    /// The enemies in your sight on the map you stand on, nearest first: foes on
    /// foot, standing ICE in a construct.
    fn enemies_in_sight(st: &AppState) -> Vec<(Enemy, Pos)> {
        let Some(p) = st.player.as_ref() else {
            return Vec::new();
        };
        match p.place {
            Place::Construct(b) | Place::SiteNet { building: b, .. } => {
                let Some(k) = st.constructs.get(b) else {
                    return Vec::new();
                };
                // Everything in a construct is in view; what the cursor can be on is
                // what a program could reach: no wall between.
                let mut out: Vec<(Enemy, Pos)> = k
                    .guards
                    .iter()
                    .enumerate()
                    .filter(|(i, g)| {
                        k.guard_at(g.pos) == Some(*i) && crate::net::line_clear_in(k, p.pos, g.pos)
                    })
                    .map(|(i, g)| (Enemy::Ice(i), g.pos))
                    .collect();
                out.sort_by_key(|(_, at)| at.chebyshev(p.pos));
                out
            }
            _ => crate::foes::in_sight(st)
                .into_iter()
                .map(|i| (Enemy::Foe(i), st.foes[i].pos))
                .collect(),
        }
    }

    /// The standing enemy on a tile of the map you stand on.
    fn enemy_at(st: &AppState, at: Pos) -> Option<Enemy> {
        let p = st.player.as_ref()?;
        match p.place {
            Place::Construct(b) | Place::SiteNet { building: b, .. } => st
                .constructs
                .get(b)
                .and_then(|k| k.guard_at(at))
                .map(Enemy::Ice),
            place => crate::foes::foe_at(st, place, at).map(Enemy::Foe),
        }
    }

    /// Whether an enemy is after you: a hunting foe, awake ICE.
    fn enemy_hunting(st: &AppState, e: Enemy) -> bool {
        match e {
            Enemy::Foe(i) => st.foes.get(i).is_some_and(|f| f.hunting()),
            Enemy::Ice(i) => st
                .player
                .as_ref()
                .and_then(|p| st.net.ice.get(&p.place.construct()?))
                .and_then(|states| states.get(i))
                .is_some_and(|ice| matches!(ice.mode, crate::net::IceMode::Awake { .. })),
        }
    }

    /// The fire cursor opens on the enemy you aimed at last if it still stands in
    /// sight, else the nearest in sight that hunts you, else the nearest at all (a
    /// dummy never steals it from a gun), or not at all.
    fn open_target(&mut self, st: &AppState) {
        let seen = Self::enemies_in_sight(st);
        let last = self
            .last_enemy
            .and_then(|e| seen.iter().find(|(s, _)| *s == e));
        self.target = last
            .or_else(|| seen.iter().find(|(e, _)| Self::enemy_hunting(st, *e)))
            .or(seen.first())
            .map(|(_, at)| *at);
    }

    /// The readout over an enemy under the cursor: its name, its weapon (or what is
    /// left of the ICE), the distance, your chance per round (or why there is none);
    /// with the characters of the chance to light when the foe is marked.
    fn aim_readout(st: &AppState, enemy: Enemy) -> (String, Option<std::ops::Range<usize>>) {
        let Some(me) = st.player.as_ref().map(|p| p.pos) else {
            return (String::new(), None);
        };
        match enemy {
            Enemy::Foe(foe) => {
                let f = &st.foes[foe];
                let chance = crate::combat::your_chance(st, foe);
                let odds = match &chance {
                    Ok(c) => format!("{c}%"),
                    Err(why) => why.clone(),
                };
                let head = format!(
                    " {} · {} · {} tiles · ",
                    f.name,
                    crate::foes::weapon_name(st, f),
                    me.chebyshev(f.pos)
                );
                let from = head.chars().count();
                let lit = (f.marked && chance.is_ok()).then(|| from..from + odds.chars().count());
                (format!("{head}{odds} "), lit)
            }
            Enemy::Ice(i) => {
                let Some(b) = st.player.as_ref().and_then(|p| p.place.construct()) else {
                    return (String::new(), None);
                };
                let Some((name, left, at)) = crate::net::ice_card(st, b, i) else {
                    return (String::new(), None);
                };
                let odds = match crate::combat::attack_chance(st, b, i) {
                    Ok(c) => format!("{c}%"),
                    Err(why) => why,
                };
                let text = format!(" {name} · {left} · {} tiles · {odds} ", me.chebyshev(at));
                (text, None)
            }
        }
    }

    /// In the Net: a construct, or the open Net, where the deck's programs are the
    /// number keys and nothing of the meat's (no throws, no pushes) works.
    fn in_net(st: &AppState) -> bool {
        st.player.as_ref().is_some_and(|p| Self::net_place(p.place))
    }

    fn net_place(place: Place) -> bool {
        place.in_construct() || place == Place::Stratum(StratumKind::Net)
    }

    /// What the act half offers, in key order: the fight's abilities (strikes,
    /// boosts, marks; content order), then the throwables in the pack; at most ten,
    /// for `1`-`9` and `0`. In a construct only what works in the net (a strike that
    /// does not push, a boost), and no throwables.
    fn act_list(st: &AppState) -> Vec<Act> {
        use game::{AbilityKind::Active, Active as A};
        let net = Self::in_net(st);
        let owned = st.character.abilities(&st.content);
        let mut out: Vec<Act> = st
            .content
            .abilities
            .iter()
            .filter(|a| owned.iter().any(|(id, _)| *id == a.id))
            .filter(|a| match a.kind {
                Active(A::Strike { push, .. }) => !net || push == 0,
                Active(A::Boost { .. }) => true,
                Active(A::Mark { .. }) => !net,
                _ => false,
            })
            .map(|a| Act::Ability(a.id.clone()))
            .collect();
        if !net {
            for s in &st.character.inventory {
                let id = &s.item;
                let thrown = st.content.item(id).is_some_and(|i| i.throw().is_some());
                if thrown && s.qty > 0 && !out.contains(&Act::Throw(id.clone())) {
                    out.push(Act::Throw(id.clone()));
                }
            }
        }
        out.truncate(10);
        out
    }

    /// The first key of the list: `1`, or `6` after a construct's programs.
    fn act_base(st: &AppState) -> u32 {
        if Self::in_net(st) { 6 } else { 1 }
    }

    /// The entry a number key picks: `0` is the tenth key, after `9`. In a construct
    /// `1`-`5` are the programs'.
    fn act_key(st: &AppState, n: u32) -> Option<Act> {
        let place = match n {
            0 => 10,
            1..=9 => n,
            _ => return None,
        };
        let i = place.checked_sub(Self::act_base(st))?;
        Self::act_list(st).get(i as usize).cloned()
    }

    /// The footer's `ACT` entries.
    fn act_entries(st: &AppState) -> Vec<hud::ActEntry> {
        use game::{AbilityKind::Active, Active as A};
        let base = Self::act_base(st);
        Self::act_list(st)
            .into_iter()
            .enumerate()
            .filter_map(|(i, act)| {
                // The keys run on from the base to `0`, the tenth, and stop there.
                let place = base + i as u32;
                if place > 10 {
                    return None;
                }
                let key = char::from_digit(place % 10, 10)?;
                Some(match act {
                    Act::Throw(id) => hud::ActEntry {
                        key,
                        name: st.content.item(&id)?.name.clone(),
                        count: Some(st.character.count(&id)),
                        state: hud::ActState::Ready,
                    },
                    Act::Ability(id) => {
                        let a = st.content.ability(&id)?;
                        let spent_boost = matches!(a.kind, Active(A::Boost { once: true }))
                            && st.net.engaged.as_ref().is_some_and(|e| e.boosted);
                        let state = if st.character.cooling(&st.content, &id, st.tick).is_some() {
                            hud::ActState::Cooling
                        } else if spent_boost {
                            hud::ActState::Refused
                        } else {
                            hud::ActState::Ready
                        };
                        hud::ActEntry {
                            key,
                            name: a.name.clone(),
                            count: None,
                            state,
                        }
                    }
                })
            })
            .collect()
    }

    /// The pools draw lit while a pulse plays on you.
    fn flash_pools(&self, st: &AppState) -> bool {
        let Some(p) = st.player.as_ref() else {
            return false;
        };
        st.fx.iter().any(|i| {
            matches!(i.fx, game::Fx::Pulse { .. })
                && i.place == p.place
                && i.origin == p.pos
                && i.started <= st.tick
                && st.tick < i.until()
        })
    }

    /// The footprint the armed item's blast would have at `at`: nothing when it
    /// cannot come down there.
    fn armed_footprint(st: &AppState, id: &game::ItemId, at: Pos) -> Vec<(Pos, i32)> {
        let (Some((range, _, blast)), Some(p), Some(city)) = (
            crate::blast::your_throw(st, id),
            st.player.as_ref(),
            st.city.as_ref(),
        ) else {
            return Vec::new();
        };
        if crate::blast::landing(st, range, at).is_err() {
            return Vec::new();
        }
        crate::blast::footprint(city, p.place, at, blast.radius)
    }

    /// The readout over the cursor with a throwable armed: the item, the distance,
    /// the blast and what it would do to each foe in it, nearest the centre first;
    /// or why it cannot land there. The flag is set when you stand in it.
    fn armed_readout(st: &AppState, id: &game::ItemId, at: Pos) -> Option<(String, bool)> {
        let item = st.content.item(id)?;
        let (range, _, blast) = crate::blast::your_throw(st, id)?;
        let p = st.player.as_ref()?;
        let name = item.name.to_lowercase();
        let dist = p.pos.chebyshev(at);
        if let Err(why) = crate::blast::landing(st, range, at) {
            return Some((format!(" {name} · {dist} tiles · {why} "), false));
        }
        let tiles = Self::armed_footprint(st, id, at);
        let mut parts = vec![
            name,
            format!("{dist} tiles"),
            format!("blast {}", blast.radius),
        ];
        for (pos, ring) in &tiles {
            let Some(foe) = crate::foes::foe_at(st, p.place, *pos).map(|i| &st.foes[i]) else {
                continue;
            };
            match crate::blast::effect(&blast, *ring, foe.chrome) {
                (0, 0) => {}
                (0, _) => parts.push(format!("{} stun", foe.name)),
                (damage, _) => parts.push(format!("{} {damage}", foe.name)),
            }
        }
        let you = tiles.iter().any(|(q, _)| *q == p.pos);
        if you {
            parts.push("you're in it".to_string());
        }
        Some((format!(" {} ", parts.join(" · ")), you))
    }

    /// The keys of your act half: a direction moves the cursor, tab the next foe,
    /// space fires (or throws what is armed), a number picks from the `ACT` list,
    /// esc puts a throwable back and then drops the cursor. `None` for a key that
    /// is not the cursor's.
    fn handle_firing(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let (place, me) = st.player.as_ref().map(|p| (p.place, p.pos))?;
        if let Some(dir) = dir_for(k.code) {
            let from = self.target.unwrap_or(me);
            let to = from.step(dir);
            if st.city.as_ref().is_some_and(|c| c.size.contains(to)) {
                self.target = Some(to);
            }
            return Some(ScreenCmd::Stay);
        }
        match k.code {
            // `t`: the next enemy. Tab stays the deck's, so a stim is a key away.
            KeyCode::Char('t') => {
                let seen = Self::enemies_in_sight(st);
                if seen.is_empty() {
                    st.warn("nothing to hit");
                    return Some(ScreenCmd::Stay);
                }
                let at = self
                    .target
                    .and_then(|t| seen.iter().position(|(_, p)| *p == t));
                let next = match at {
                    Some(i) => (i + 1) % seen.len(),
                    None => 0,
                };
                self.target = Some(seen[next].1);
                Some(ScreenCmd::Stay)
            }
            // Esc puts a throwable back first; the cursor stays.
            KeyCode::Esc if self.armed.is_some() => {
                self.armed = None;
                Some(ScreenCmd::Stay)
            }
            KeyCode::Esc if self.target.is_some() => {
                self.target = None;
                Some(ScreenCmd::Stay)
            }
            KeyCode::Char(' ') if self.armed.is_some() => {
                let id = self.armed.clone()?;
                match crate::blast::throw(st, &id, self.target.unwrap_or(me)) {
                    // A throw that cannot come down there leaves it armed to aim again.
                    Err(why)
                        if matches!(why.as_str(), "out of range" | "no line" | "no landing") =>
                    {
                        st.warn(why)
                    }
                    Err(why) => {
                        self.armed = None;
                        st.warn(why);
                    }
                    Ok(()) => self.armed = None,
                }
                Some(ScreenCmd::Stay)
            }
            KeyCode::Char(' ') => {
                // A program armed at something keeps space as its roll; the cursor's
                // attack is for when nothing is in flight.
                if Self::net_place(place) && st.net.run.is_some() {
                    return None;
                }
                match self.target.and_then(|t| Self::enemy_at(st, t)) {
                    // ICE on the open Net takes the deck's attack, not a gun.
                    Some(Enemy::Foe(foe)) if st.foes.get(foe).is_some_and(|f| f.ice.is_some()) => {
                        self.last_enemy = Some(Enemy::Foe(foe));
                        match crate::combat::attack_grid(st, foe) {
                            Ok(slot) => self.program_flash = Some((slot, st.tick)),
                            Err(why) => st.warn(why),
                        }
                    }
                    Some(Enemy::Foe(foe)) => {
                        self.last_enemy = Some(Enemy::Foe(foe));
                        if let Err(why) = crate::combat::fire(st, foe) {
                            st.warn(why);
                        }
                    }
                    Some(Enemy::Ice(i)) => {
                        self.last_enemy = Some(Enemy::Ice(i));
                        let Some(b) = place.construct() else {
                            return Some(ScreenCmd::Stay);
                        };
                        match crate::combat::attack_ice(st, b, i) {
                            Ok(slot) => self.program_flash = Some((slot, st.tick)),
                            Err(why) => st.warn(why),
                        }
                    }
                    // In a construct space keeps its other jobs: a run in flight
                    // rolls, a node opens.
                    None if Self::net_place(place) => return None,
                    None => st.warn("pick a target"),
                }
                Some(ScreenCmd::Stay)
            }
            KeyCode::Char(c) => {
                let n = c.to_digit(10)?;
                let Some(act) = Self::act_key(st, n) else {
                    // On foot a number with nothing behind it does nothing; in a
                    // construct 1-5 are the deck's programs.
                    return (!Self::net_place(place)).then_some(ScreenCmd::Stay);
                };
                match act {
                    Act::Throw(id) => {
                        self.target.get_or_insert(me);
                        self.armed = Some(id);
                    }
                    Act::Ability(id) => {
                        self.armed = None;
                        self.use_ability(st, place, &id);
                    }
                }
                Some(ScreenCmd::Stay)
            }
            _ => None,
        }
    }

    /// An ability of the `ACT` list fired: a strike or a mark at the enemy under the
    /// cursor, a boost at once. The engine says why not.
    fn use_ability(&mut self, st: &mut AppState, place: Place, id: &game::AbilityId) {
        use game::{AbilityKind::Active, Active as A};
        let Some(kind) = st.content.ability(id).map(|a| a.kind.clone()) else {
            return;
        };
        let enemy = self.target.and_then(|t| Self::enemy_at(st, t));
        let result = match (kind, enemy, place) {
            (Active(A::Boost { .. }), _, _) => crate::casts::boost(st, id),
            (Active(A::Strike { .. }), Some(Enemy::Foe(foe)), _) => {
                self.last_enemy = enemy;
                crate::combat::strike(st, id, foe)
            }
            (
                Active(A::Strike { .. }),
                Some(Enemy::Ice(i)),
                Place::Construct(b) | Place::SiteNet { building: b, .. },
            ) => {
                self.last_enemy = enemy;
                crate::casts::strike_ice(st, id, b, i)
            }
            (Active(A::Mark { .. }), Some(Enemy::Foe(foe)), _) => {
                self.last_enemy = enemy;
                crate::casts::mark(st, id, foe)
            }
            _ => Err("pick a target".to_string()),
        };
        if let Err(why) = result {
            st.warn(why);
        }
    }

    /// A step of yours on turns on foot: a move spent, the last one opening the
    /// act half.
    fn engaged_step(&mut self, st: &mut AppState, dir: Dir) -> ScreenCmd {
        // Off the city's cells: the fight holds you in.
        if let Some(cmd) = self.off_the_edge(st, dir) {
            return cmd;
        }
        // A blocked step costs nothing.
        if !open_step(st, dir) {
            say_why_not(st, dir);
            return ScreenCmd::Stay;
        }
        if let Err(why) = crate::net::spend_move(st) {
            st.warn(why);
            return ScreenCmd::Stay;
        }
        if step(st, dir) {
            self.hand_over(st);
        }
        if st.net.engaged.as_ref().is_some_and(|e| e.moves == 0) {
            crate::net::stay(st);
        }
        ScreenCmd::Stay
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

    /// The route row's two halves: the route you came by while jacked in on a rig run,
    /// and the trace walking home, wherever you stand.
    fn route_parts(&self, st: &AppState, jacked: bool) -> (String, Option<String>) {
        let name = |b| crate::helix::hosts::building_hostname(st, b);
        let route = match st.backdoors.route.as_ref() {
            Some(r) if jacked => {
                let names: Vec<String> = r.relays.iter().map(|b| name(*b)).collect();
                crate::backdoors::route_line(&names)
            }
            _ => String::new(),
        };
        let trace = st.backdoors.trace_line(st.net.now(st), &name);
        (route, trace)
    }

    /// What the footer reads this frame. `tethered`: the run hangs off its target on
    /// the map, so the footer leaves it out.
    fn footer<'a>(
        &self,
        st: &AppState,
        city: &citygen::City,
        jacked: bool,
        slots: &'a [hud::Slot],
        tethered: bool,
    ) -> hud::Footer<'a> {
        let place = match (jacked, st.player.as_ref().map(|p| p.place)) {
            (true, Some(Place::Construct(b) | Place::SiteNet { building: b, .. })) => {
                let district = city
                    .building_opt(b)
                    .map(|bl| city.district(bl.district).name.clone())
                    .unwrap_or_else(|| city.name.clone());
                if city.is_lot() {
                    "the lot · construct".to_string()
                } else {
                    format!("{} · {district}", crate::net::building_name(st, b))
                }
            }
            (true, Some(_)) => {
                // off the live city's cells, the Net is named as HERE names it
                let pos = st.player.as_ref().map(|q| q.pos).unwrap_or(Pos::new(0, 0));
                let name = match Beyond::of(st) {
                    Some(b) if !b.live_owns(pos) => b.net_name(pos),
                    _ => crate::net::district_name(city, pos),
                };
                format!("{name} · grid")
            }
            _ => self.place_label(st),
        };
        let now = st.net.now(st);
        let trace = match (jacked, st.player.as_ref()) {
            (true, Some(p)) => Some(match p.place {
                Place::Construct(b) | Place::SiteNet { building: b, .. } => {
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
            (true, Some(run), Some(Place::Construct(b) | Place::SiteNet { building: b, .. }))
                if !tethered =>
            {
                crate::net::run_readout(st, b, run)
            }
            (true, Some(run), Some(Place::Stratum(StratumKind::Net))) if !tethered => {
                crate::net::run_readout_at(st, crate::net::Scene::Outside, run)
            }
            _ => None,
        };
        let ice = match (jacked, st.player.as_ref()) {
            (true, Some(p)) => match p.place {
                place if place.in_construct() => {
                    let vis = p.vis();
                    crate::net::ice_lines(st, place, |q| {
                        vis.get(q)
                            .is_some_and(|v| *v != crate::render::Visibility::Unseen)
                    })
                }
                _ => Vec::new(),
            },
            (false, Some(p)) => {
                let vis = p.vis();
                crate::foes::lines(st, |q| {
                    vis.get(q)
                        .is_some_and(|v| *v != crate::render::Visibility::Unseen)
                })
            }
            _ => Vec::new(),
        };
        hud::Footer {
            place,
            saved: crate::runs::when::marked(st),
            jacked,
            camera: self.mode == Mode::Explore,
            overburdened: st.character.overburdened(&st.content),
            trace,
            hunted: jacked && crate::net::hunted(st),
            engaged: crate::net::engaged_on_foot(st),
            turn: crate::net::turn_readout(st),
            run,
            ice,
            slots_label: if jacked { "PROGRAMS" } else { "ABILITIES" },
            slots,
            hollow: !crate::net::your_turn(st)
                || crate::net::phase(st) == Some(crate::net::Phase::Move),
            acting: crate::net::phase(st).map(|p| p == crate::net::Phase::Act),
            prompt: self.turn_prompt(st),
            act: (self.firing(st) && crate::net::your_turn(st)).then(|| Self::act_entries(st)),
            flash_pools: self.flash_pools(st),
            budgets: st.net.engaged.as_ref().map(|e| {
                let moves = crate::net::you_move(st, e.place);
                if e.actor() == crate::net::Actor::You {
                    (e.moves, moves, e.ap, crate::net::BASE_AP)
                } else {
                    (moves, moves, crate::net::BASE_AP, crate::net::BASE_AP)
                }
            }),
        }
    }

    /// Row 2 of the footer on turns: whose turn, which half, and the keys for it.
    fn turn_prompt(&self, st: &AppState) -> Option<(String, String)> {
        use crate::net::{Actor, Phase};
        if self.looking.is_some() {
            return Some((
                "LOOK".to_string(),
                "arrows move · tab next · esc back".to_string(),
            ));
        }
        if let Some(w) = self.which_way.as_ref() {
            let mut parts: Vec<String> = w
                .subjects
                .iter()
                .filter_map(|s| s.dir.map(|d| format!("{} {}", arrow(d), s.name())))
                .collect();
            if let Some(here) = w.subjects.iter().find(|s| s.dir.is_none()) {
                parts.push(match &here.header {
                    Some(h) => format!("· here: {}", h.name),
                    None => "· here".to_string(),
                });
            }
            return Some(("WHICH WAY?".to_string(), parts.join("   ")));
        }
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
        let on_foot = crate::net::engaged_on_foot(st);
        Some(match e.actor() {
            Actor::You if on_foot => match e.phase {
                Phase::Move => (
                    "YOUR TURN · MOVE".to_string(),
                    "hjkl step · g aims a path · enter walks it · space stays".to_string(),
                ),
                Phase::Act if self.armed.is_some() => (
                    "THROW".to_string(),
                    "hjkl aim · space throw · esc back".to_string(),
                ),
                Phase::Act => (
                    "AIM".to_string(),
                    "hjkl target · t next · space fire · 1-9 act · . end turn".to_string(),
                ),
            },
            Actor::You => match e.phase {
                Phase::Move => (
                    "YOUR TURN · MOVE".to_string(),
                    "arrows aim · space or enter walks · space on yourself stays".to_string(),
                ),
                // Act: the cursor on a piece and space attacks it; a program's key
                // picks a program (and a target when several fit) and space runs
                // it, an action point each; `.` ends the turn.
                Phase::Act => (
                    "AIM".to_string(),
                    match st
                        .net
                        .run
                        .as_ref()
                        .and_then(|r| crate::net::run_label(st, r))
                        .filter(|_| self.target.and_then(|t| Self::enemy_at(st, t)).is_none())
                    {
                        Some(label) => {
                            format!("space runs {label} (1 AP) · 1-5 another program · . end turn")
                        }
                        None => "hjkl target · t next · space attack · 1-5 program · . end turn"
                            .to_string(),
                    },
                ),
            },
            Actor::Ice(i) => {
                let grade = e
                    .construct()
                    .and_then(|b| st.constructs.get(b))
                    .and_then(|k| k.guards.get(i))
                    .map(|g| g.label())
                    .unwrap_or("the");
                (format!("{grade} ICE ACTS"), "wait".to_string())
            }
            Actor::Foe(i) => {
                let name = st
                    .foes
                    .get(i)
                    .map(|f| f.name.to_uppercase())
                    .unwrap_or_else(|| "FOE".to_string());
                (format!("THE {name} ACTS"), "wait".to_string())
            }
            Actor::Bot(site, n) => {
                let name = crate::bots::name(st, (site, n))
                    .map(|n| n.to_uppercase())
                    .unwrap_or_else(|| "BOT".to_string());
                (format!("YOUR {name} MOVES"), "wait".to_string())
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
        ability_slots(st)
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
                    format!(
                        "{mark} {}  F{}",
                        crate::ui::hud::capitalised(s.label()),
                        i + 1
                    ),
                    Pick::View(*s),
                )
            })
            .collect();
        ScreenCmd::Push {
            to: Box::new(Popup::verbs("View", choices, st.tick)),
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
        MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            self.place,
            st.minute_of_day(),
        )
        .unwrap_or_else(|| MapRef::stratum_of(city, StratumKind::Street))
    }

    /// Works out where the occupants stand when the player is on a floor; once per
    /// place and world minute. Nobody stands on a mission's copy: the real building
    /// carries on unseen.
    fn refresh_placement(&mut self, st: &AppState) {
        let Some(p) = st.player.as_ref() else {
            self.placement = None;
            return;
        };
        let Place::Floor { building, level } = p.place else {
            self.placement = None;
            return;
        };
        if crate::venues::copy_live(st, building) {
            self.placement = None;
            return;
        }
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
        let homes = st.interiors.homes_of(building).cloned().unwrap_or_default();
        let Some((plan, floor)) = st.interiors.floor(building, level) else {
            return;
        };
        self.placement = Some((
            p.place,
            minute,
            crate::interiors::Placement::new(plan, floor, world, &homes),
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
            (Place::Stratum(_), Some(c)) if c.is_lot() => c.name.clone(),
            (Place::Stratum(k), _) => k.label().to_string(),
            (Place::Floor { level, .. }, _) => citygen::FloorPlan::label(level),
            (Place::Construct(_) | Place::SiteNet { .. }, _) => "construct".to_string(),
        }
    }

    fn enter_walk(&mut self, st: &mut AppState) {
        let bonus = st.sight_bonus();
        let minute = st.minute_of_day();
        let Some(city) = st.city.as_ref() else { return };
        if st.player.is_none() {
            let home = st
                .home
                .and_then(|rig| rig.wake(city, &mut st.interiors, &st.constructs, bonus));
            st.player = Some(home.unwrap_or_else(|| {
                let mut p = Player::spawn(city, hash64(city.seed, st.tick));
                p.update_fov(
                    &MapRef::stratum_at(city, StratumKind::Street, minute),
                    bonus,
                );
                p
            }));
            if let Some(rig) = st.home {
                crate::credentials::prepare(st, rig.home.building);
                crate::credentials::move_in(st);
            }
        }
        if let Some(p) = &st.player {
            self.place = p.place;
            self.cam.snap(p.pos);
        }
        self.mode = Mode::Walk;
    }

    fn handle_common(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        match k.code {
            KeyCode::Char('`') if st.dev_unlocked => {
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
            // The key list's last line: space turns to the manual's Controls chapter.
            KeyCode::Char(' ') if self.help => {
                self.help = false;
                return Some(ScreenCmd::Push {
                    to: Box::new(crate::screens::manual::Manual::at_chapter("controls")),
                    fx: TransitionKind::None,
                });
            }
            KeyCode::Tab => {
                // off the live city's cells your mark is off its map: the MAP is the
                // world's, as it is out on the overworld
                let focus = self.focus_pos(st);
                if carries_on(st, self.place) && !crate::outside::stream::live_owns(st, focus) {
                    st.atlas.zoom = crate::screens::atlas::Zoom::World;
                    st.atlas.world.cursor = None;
                }
                return Some(ScreenCmd::Push {
                    to: Box::new(Deck::new(st.tick).focus(self.place, focus).restore(st)),
                    fx: TransitionKind::Glitch,
                });
            }
            KeyCode::Char('s') => {
                if let Some(why) = crate::save::refusal(st) {
                    st.warn(why);
                } else {
                    crate::runs::when::save_now(st);
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.help {
                    self.help = false;
                } else if k.code == KeyCode::Esc && st.net.feed.take().is_some() {
                    // Esc closes a camera's feed before it opens the menu.
                    st.say("feed closed");
                } else {
                    return Some(ScreenCmd::Push {
                        to: Box::new(Pause::new(st, st.tick)),
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
                return Some(crate::screens::start_city(st, params));
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

    /// Feeds keys to the open console; a run line goes through the commands, and one
    /// that opens a window closes the console and pushes it.
    fn feed_console(
        &mut self,
        st: &mut AppState,
        focus: game::At,
        keys: &[KeyEvent],
    ) -> Option<ScreenCmd> {
        let con = self.console.as_mut()?;
        let mut close = false;
        let mut opened = None;
        for k in keys {
            match con.key(*k, &|head, word| console::complete(st, head, word)) {
                ConsoleEvent::Run(line) => {
                    let result = console::run(st, focus, &line);
                    if result.is_ok() {
                        opened = console::opens(st, &line);
                    }
                    report(st, result);
                    if opened.is_some() {
                        close = true;
                        break;
                    }
                }
                ConsoleEvent::Close => close = true,
                ConsoleEvent::None => {}
            }
        }
        if close {
            self.console = None;
        }
        opened.map(|to| ScreenCmd::Push {
            to,
            fx: TransitionKind::None,
        })
    }

    /// What Space opens: one thing in reach goes straight to its window, none gives a
    /// window with Look alone; a construct lists the programs that apply.
    fn interact(&mut self, st: &AppState) -> Option<ScreenCmd> {
        let player = st.player.as_ref()?;
        let (place, pos) = (player.place, player.pos);
        let look = Choice::new('x', "Look", Pick::Look(pos));
        let subjects = interact::gather(st, &|pl, p| self.inside_person_at(pl, p));
        // On the Net: what a held node near you does, then what the deck's programs
        // reach, after whatever else is here (the port's Jack out). In a construct
        // always; on the open Net when there is any and at most one thing is here.
        if let Some(scene) = crate::net::scene_of(place)
            && st.character.deck(&st.content).is_some()
        {
            let inside = place.in_construct();
            let here = match (inside, subjects.as_slice()) {
                (false, [s]) => Some(s),
                _ => None,
            };
            let verbs = interact::verb_rows(st);
            let taken: Vec<char> = here
                .map(|s| s.choices.iter().map(|c| c.key).collect())
                .unwrap_or_default();
            let free: String = FREE_LETTERS
                .chars()
                .filter(|c| !verbs.iter().any(|v| v.key == *c) && !taken.contains(c))
                .collect();
            let mut letters = free.chars();
            let rows = interact::run_rows_at(st, scene, pos, None, &mut letters);
            let fits = inside || subjects.len() <= 1;
            if fits && (inside || !verbs.is_empty() || !rows.is_empty()) {
                let popup = match here {
                    Some(s) => {
                        let mut choices = vec![Choice::new('x', "Look", Pick::Look(s.pos))];
                        choices.extend(s.choices.iter().cloned());
                        choices.extend(verbs);
                        choices.extend(rows);
                        Popup::about(s.header.clone(), s.subtitle.clone(), choices, st.tick)
                            .with_body(s.body.clone())
                    }
                    None => {
                        let mut choices = vec![look];
                        choices.extend(verbs);
                        choices.extend(rows);
                        Popup::verbs("", choices, st.tick)
                    }
                };
                return Some(ScreenCmd::Push {
                    to: Box::new(popup),
                    fx: TransitionKind::None,
                });
            }
        }
        let popup = match subjects.as_slice() {
            [] => Popup::verbs("", vec![look], st.tick),
            [s] => s.window(st.tick),
            _ => {
                self.which_way = Some(WhichWay {
                    subjects,
                    from: (place, pos),
                });
                return Some(ScreenCmd::Stay);
            }
        };
        Some(ScreenCmd::Push {
            to: Box::new(popup),
            fx: TransitionKind::None,
        })
    }

    /// A verb key jacked in: a line logged, the hijack's picker, or a held terminal's
    /// shell; out of reach or spent, a warning.
    fn verb_key(st: &mut AppState, c: char) -> ScreenCmd {
        match crate::screens::popup::verb_opens(st, c) {
            Some(to) => ScreenCmd::Push {
                to,
                fx: TransitionKind::None,
            },
            None => ScreenCmd::Stay,
        }
    }

    /// A number key jacked in: the deck's program in that slot, run at once against
    /// the one target it applies to, or the short list when there are several.
    fn program_key(&mut self, st: &mut AppState, n: usize) -> ScreenCmd {
        let Some((place, pos)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
            return ScreenCmd::Stay;
        };
        let Some(scene) = crate::net::scene_of(place) else {
            st.warn("programs run on the Net");
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
        for (target, what) in crate::net::targets_at(st, scene, pos) {
            let Some(kind) = crate::net::kind_at(st, scene, target) else {
                continue;
            };
            if !game::applies(spec.op, kind) {
                continue;
            }
            if spec.op == game::Op::Copy && st.net.copied(target, now) {
                copied = true;
                continue;
            }
            let Some(at) = crate::net::pos_at(st, scene, target) else {
                continue;
            };
            candidates.push((target, at, what));
        }
        if candidates.is_empty() {
            // another city's node it would run against: out of your body's reach
            let far = scene == crate::net::Scene::Outside
                && crate::net::out_of_reach(st, pos)
                    .into_iter()
                    .any(|k| game::applies(spec.op, game::Target::Node(k)));
            st.warn(if far {
                crate::net::TOO_FAR.to_string()
            } else if copied {
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

    /// Keys while Space asks which way: a direction with something there opens its
    /// window, space or `.` opens your own tile, esc puts the pick away.
    fn handle_which_way(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let Some(w) = self.which_way.as_ref() else {
            return ScreenCmd::Stay;
        };
        let pick = match k.code {
            KeyCode::Esc => {
                self.which_way = None;
                return ScreenCmd::Stay;
            }
            KeyCode::Char(' ' | '.') => w.subjects.iter().find(|s| s.dir.is_none()),
            code => match dir_for(code) {
                Some(d) => w.subjects.iter().find(|s| s.dir == Some(d)),
                None => None,
            },
        };
        let Some(s) = pick else {
            return ScreenCmd::Stay;
        };
        let to = Box::new(s.window(st.tick));
        self.which_way = None;
        ScreenCmd::Push {
            to,
            fx: TransitionKind::None,
        }
    }

    /// The map tiles on screen.
    fn view_rect(&self) -> TileRect {
        let map = self.last_map.get();
        let origin = self.cam.origin(map.width, map.height);
        TileRect::new(origin.x, origin.y, map.width as i32, map.height as i32)
    }

    /// Keys in look mode: the walk keys move the cursor (shift by eight), tab and
    /// shift-tab step through what is worth a look, space opens what is in reach,
    /// esc or `x` puts the cursor away.
    fn handle_looking(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let Some(cursor) = self.looking.as_ref().map(|l| l.cursor) else {
            return ScreenCmd::Stay;
        };
        let fast = k.modifiers.contains(KeyModifiers::SHIFT)
            || matches!(
                k.code,
                KeyCode::Char('H' | 'J' | 'K' | 'L' | 'Y' | 'U' | 'B' | 'N')
            );
        let code = match k.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        let moved = match code {
            KeyCode::Esc | KeyCode::Char('x') => {
                self.looking = None;
                return ScreenCmd::Stay;
            }
            KeyCode::Char(' ') => {
                // another city's node beside you on the open Net: out of your body's reach
                let far = st.player.as_ref().is_some_and(|p| {
                    p.place == Place::Stratum(StratumKind::Net)
                        && p.pos.chebyshev(cursor) <= 1
                        && !crate::net::out_of_reach_on(st, std::iter::once(cursor)).is_empty()
                });
                if far {
                    st.warn(crate::net::TOO_FAR);
                    return ScreenCmd::Stay;
                }
                return self.open_looked_at(st, cursor);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let Some(city) = st.city.as_ref() else {
                    return ScreenCmd::Stay;
                };
                let from = st.player.as_ref().map_or(cursor, |p| p.pos);
                let map = self.map(st, city);
                let pois = crate::look::points_of_interest(
                    st,
                    &map,
                    self.place,
                    self.view_rect(),
                    from,
                    &|pl, p| self.inside_person_at(pl, p),
                );
                let n = pois.len();
                if n == 0 {
                    return ScreenCmd::Stay;
                }
                let at = pois.iter().position(|p| *p == cursor);
                let next = if code == KeyCode::Tab {
                    at.map_or(0, |i| (i + 1) % n)
                } else {
                    at.map_or(n - 1, |i| (i + n - 1) % n)
                };
                pois[next]
            }
            c => match dir_for(c) {
                Some(d) => {
                    let step = if fast { 8 } else { 1 };
                    let (dx, dy) = d.delta();
                    let view = self.view_rect();
                    // the view bounds the cursor, and the map too where it ends; the
                    // street and the Net carry on past the city's edge
                    let size = st
                        .city
                        .as_ref()
                        .filter(|_| !carries_on(st, self.place))
                        .map(|city| TileMap::size(&self.map(st, city)));
                    let (lo, (w, h)) = match size {
                        Some(s) => (0, (s.w as i32, s.h as i32)),
                        None => (i32::MIN, (i32::MAX, i32::MAX)),
                    };
                    let lo_x = view.x.max(lo);
                    let hi_x = (view.x + view.w - 1).min(w - 1).max(lo_x);
                    let lo_y = view.y.max(lo);
                    let hi_y = (view.y + view.h - 1).min(h - 1).max(lo_y);
                    Pos::new(
                        (cursor.x + dx * step).clamp(lo_x, hi_x),
                        (cursor.y + dy * step).clamp(lo_y, hi_y),
                    )
                }
                None => return ScreenCmd::Stay,
            },
        };
        if let Some(l) = self.looking.as_mut() {
            l.cursor = moved;
        }
        ScreenCmd::Stay
    }

    /// Space in look mode: the window of what is under the cursor, when it is in
    /// reach; in a construct, the programs that apply to it.
    fn open_looked_at(&mut self, st: &AppState, cursor: Pos) -> ScreenCmd {
        let Some((place, pos)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
            return ScreenCmd::Stay;
        };
        let popup = if let Some(b) = place.construct() {
            let mut letters = FREE_LETTERS.chars();
            let rows: Vec<Choice> = interact::run_rows(st, b, pos, None, &mut letters)
                .into_iter()
                .filter(|c| {
                    matches!(c.pick, Pick::Run(_, t) if crate::net::target_pos(st, b, t) == Some(cursor))
                })
                .collect();
            if rows.is_empty() {
                return ScreenCmd::Stay;
            }
            let mut choices = vec![Choice::new('x', "Look", Pick::Look(cursor))];
            choices.extend(rows);
            Popup::verbs("", choices, st.tick)
        } else {
            let subjects = interact::gather(st, &|pl, p| self.inside_person_at(pl, p));
            match subjects.iter().find(|s| s.pos == cursor) {
                Some(s) => s.window(st.tick),
                None => return ScreenCmd::Stay,
            }
        };
        self.looking = None;
        ScreenCmd::Push {
            to: Box::new(popup),
            fx: TransitionKind::None,
        }
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
        let mut letters = FREE_LETTERS.chars();
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
            to: Box::new(Popup::verbs("Abilities", choices, st.tick)),
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
        // A standing foe holds its tile on any map.
        if let Some(i) = crate::foes::foe_at(st, place, target) {
            let name = st.foes[i].name.clone();
            st.warn(format!("the {name} is in the way"));
            return Err(ScreenCmd::Stay);
        }
        // A visitor holds their tile too.
        if let Some(v) = crate::missions::visitors::at(st, place, target) {
            let name = crate::missions::role_name(st, &v.role);
            st.warn(format!("`{name}` is in the way"));
            return Err(ScreenCmd::Stay);
        }
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
    /// a locked door or a grille you can't crawl through, nowhere.
    fn doorway(&mut self, st: &mut AppState, dir: Dir) -> Option<ScreenCmd> {
        let player = st.player.as_ref()?;
        let (here, target, place) = (player.pos, player.pos.step(dir), player.place);
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
                let exit = st
                    .interiors
                    .floor(building, level)?
                    .1
                    .exit_at(target)
                    .copied();
                if let Some(exit) = exit {
                    let result = crate::interiors::leave(st, &exit);
                    report(st, result);
                    return Some(ScreenCmd::Stay);
                }
                // A venue's lock or grille, before any room's door.
                match crate::venues::step_onto(st, building, level, here, target) {
                    crate::venues::Meets::Held(why) => {
                        st.warn(why);
                        return Some(ScreenCmd::Stay);
                    }
                    crate::venues::Meets::Through(line) => {
                        if let Some(line) = line {
                            st.say(line);
                        }
                        return None;
                    }
                    crate::venues::Meets::Nothing => {}
                }
                let (_, floor) = st.interiors.floor(building, level)?;
                let room = floor
                    .door_of(target)
                    .filter(|r| {
                        r.private && r.kind != citygen::RoomKind::Home && !r.rect.contains(here)
                    })
                    .cloned();
                // Your own door opens for you; the step onto it from outside names the flat.
                let home = floor
                    .door_of(target)
                    .filter(|r| r.kind == citygen::RoomKind::Home && !r.rect.contains(here))
                    .map(|r| r.number);
                if let Some(r) = room {
                    use crate::credentials::How;
                    let how = crate::credentials::opens(st, building, level, r.number);
                    let owner = st
                        .interiors
                        .homes_of(building)
                        .and_then(|h| h.owner_of(level, r.number))
                        .or_else(|| {
                            self.placement
                                .as_ref()
                                .filter(|(pl, _, _)| *pl == place)
                                .and_then(|(_, _, p)| p.owner_of(target))
                        })
                        .and_then(|id| {
                            st.world
                                .as_ref()
                                .map(|w| w.people()[id.0 as usize].name.clone())
                        });
                    let mut what = r.kind.label().to_string();
                    if r.number > 0 {
                        // A venue room by its label: B1-04 below the street.
                        let number = crate::credentials::venue_door(st, building, level, r.number)
                            .unwrap_or_else(|| r.number.to_string());
                        what = format!("{what} {number}");
                    }
                    let (whose, lit) = match &owner {
                        Some(name) => (format!("{name}'s {what}"), format!("`{name}`'s {what}")),
                        None => (what.clone(), what.clone()),
                    };
                    match how {
                        None => {
                            st.warn(format!("locked · {whose}"));
                            return Some(ScreenCmd::Stay);
                        }
                        Some(How::Cracked | How::Free) => {}
                        Some(How::Keycard) => st.say(format!("{lit} · the keycard opens it")),
                        Some(How::Code(c)) => st.say(format!("{lit} · you key in {c:04}")),
                    }
                }
                if let Some(n) = home {
                    st.say(format!("Entering `home` · flat {n}"));
                }
                None
            }
            // The gate in and out of a construct comes with the construct cache.
            Place::Construct(_) => None,
            // On a site's Net a door's node or a turnstile, as its lock on the floor.
            Place::SiteNet { .. } => match crate::sitenet::step_on_net(st, here, target) {
                crate::venues::Meets::Held(why) => {
                    st.warn(why);
                    Some(ScreenCmd::Stay)
                }
                crate::venues::Meets::Through(line) => {
                    if let Some(line) = line {
                        st.say(line);
                    }
                    None
                }
                crate::venues::Meets::Nothing => None,
            },
        }
    }

    /// A construct's gate on the Net, stepped onto from the open grid, leads inside; the
    /// entry inside, stepped off outward, leads back onto the grid.
    /// A step along a lane of the open Net, out of a fight: the lane carries you on to
    /// its next stop, past the city's edge too (the open Net's trunks, another city's or
    /// town's lanes). Whether a ride started.
    fn start_ride(&mut self, st: &mut AppState, dir: Dir) -> bool {
        // Riding the lanes is the Data surfer talent's; without it a lane is walked.
        if st.net.engaged.is_some() || !crate::ride::surfs(st) {
            return false;
        }
        let Some(p) = st.player.as_ref() else {
            return false;
        };
        if p.place != Place::Stratum(StratumKind::Net) {
            return false;
        }
        let (path, runs_on) = ride_over(st, p.pos, dir);
        if path.is_empty() {
            return false;
        }
        self.riding = Some(crate::ride::Ride {
            path,
            done: Vec::new(),
            next_at: st.clock_tick(),
            runs_on,
        });
        // A ride is a move: the run in flight is dropped, a feed closed.
        if st.net.run.take().is_some() {
            st.say("run aborted");
        }
        if st.net.feed.take().is_some() {
            st.say("feed closed");
        }
        true
    }

    /// The ride's tiles whose time has come, taken in turn over the Net past the city's
    /// edge too, the live city following you over a line as a step does; it ends at its
    /// stop, at a line it may not cross now (saying why), or when you are no longer on
    /// the open Net. One that stopped short of the Net not built yet runs on once it
    /// has come in.
    fn ride_step(&mut self, st: &mut AppState) {
        const NET: Place = Place::Stratum(StratumKind::Net);
        let now = st.clock_tick();
        let mut ridden = Vec::new();
        while let Some(ride) = self.riding.as_mut() {
            let Some(here) = st.player.as_ref().filter(|p| p.place == NET).map(|p| p.pos) else {
                self.riding = None;
                break;
            };
            if now < ride.next_at {
                break;
            }
            if ride.path.is_empty() {
                // stopped short of the Net not built yet: on from where it stopped, over
                // what has come in since (the next frame's, built round you first)
                if !ridden.is_empty() {
                    break;
                }
                self.ride_again(st);
                continue;
            }
            let next = ride.path[0];
            if let Some(why) = crossing_refused(st, here, next) {
                self.riding = None;
                st.warn(why);
                break;
            }
            if !open_at(st, NET, next) {
                self.riding = None;
                break;
            }
            ride.path.remove(0);
            ride.done.push(here);
            ride.next_at += crate::ride::RIDE_TICKS;
            // at its stop it ends, unless it stopped short of the Net not built yet
            if ride.path.is_empty() && !ride.runs_on {
                self.riding = None;
            }
            if let Some(p) = st.player.as_mut() {
                p.pos = next;
            }
            ridden.push(next);
            self.hand_over(st);
        }
        if !ridden.is_empty() {
            look(st);
        }
        // Slipstream: each tile ridden, a chance the trace loses some of you.
        for at in ridden {
            crate::ride::slipstream(st, at);
        }
    }

    /// The ride in flight found again from where it is, over the Net as it is now: a
    /// city or town came in under it (built in the background, or on the spot at its
    /// line), whose own lanes needn't run where the stream stood in for them, or more of
    /// the open Net was built past a stop that was only its edge. The ride ends where
    /// you stand when the Net there has no way on.
    fn ride_again(&mut self, st: &AppState) {
        let Some(ride) = self.riding.as_mut() else {
            return;
        };
        let Some(here) = st
            .player
            .as_ref()
            .filter(|p| p.place == Place::Stratum(StratumKind::Net))
            .map(|p| p.pos)
        else {
            return;
        };
        // on from the tile it came off, or the way it set off from where you stand
        let (from, to) = match (ride.done.last(), ride.path.first()) {
            (Some(came), _) => (*came, here),
            (None, Some(next)) => (here, *next),
            (None, None) => return,
        };
        let (path, runs_on) = Dir::CARDINAL
            .into_iter()
            .find(|d| from.step(*d) == to)
            .map(|d| ride_over(st, from, d))
            .unwrap_or_default();
        let ahead: Vec<Pos> = path.into_iter().skip(usize::from(from != here)).collect();
        // it stops short of a tile it has ridden: a loop of lane is ridden once
        let fresh = ahead
            .iter()
            .position(|q| ride.done.contains(q))
            .unwrap_or(ahead.len());
        ride.runs_on = runs_on && fresh == ahead.len();
        ride.path = ahead;
        ride.path.truncate(fresh);
        if ride.path.is_empty() {
            self.riding = None;
        }
    }

    fn gateway(&mut self, st: &mut AppState, dir: Dir) -> Option<ScreenCmd> {
        // A gate of another city's Net: walked up to, not gone through.
        if foreign_gate(st, dir) {
            st.warn(crate::net::TOO_FAR);
            return Some(ScreenCmd::Stay);
        }
        let player = st.player.as_ref()?;
        let (here, target) = (player.pos, player.pos.step(dir));
        let city = st.city.as_ref()?;
        match player.place {
            Place::Stratum(StratumKind::Net) => {
                // Onto the gate from the grid, or from the gate into the construct's bulk.
                // the live city's own, on the cells it owns: on another's, that one's
                // map is what's there
                let gate_of = |p: Pos| {
                    city.tile(StratumKind::Net, p)
                        .filter(|t| t.terrain == citygen::Terrain::NetGate)
                        .and_then(|t| t.building)
                        .filter(|_| crate::outside::stream::live_owns(st, p))
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
            // Out through the entry: a step off it outward; on a site's Net the step onto
            // it as well (its firewall's gate, which lets you out once every lock is
            // broken, else holds you on it).
            Place::Construct(b) | Place::SiteNet { building: b, .. } => {
                let k = st.constructs.get(b)?;
                let onto = k.is_site() && k.entry == target;
                if !onto && !crate::constructs::leads_out(k, here, target) {
                    return None;
                }
                if onto && !step(st, dir) {
                    say_why_not(st, dir);
                    return Some(ScreenCmd::Stay);
                }
                let result = crate::constructs::leave_construct(st);
                report(st, result);
                self.follow_player(st);
                Some(ScreenCmd::Stay)
            }
            _ => None,
        }
    }

    /// On foot or your Net self on a cell another city or town owns: the run handed
    /// over to it in place (`outside::stream::hand_over`), and everything the screen
    /// holds in the city's cells moved with the frame, so nothing on screen jumps and
    /// nothing changes.
    fn hand_over(&mut self, st: &mut AppState) {
        let Some(d) = crate::outside::stream::hand_over(st) else {
            return;
        };
        let by = |p: Pos| p.offset(d.x, d.y);
        self.cam.shift(d);
        self.cursor = by(self.cursor);
        self.aim = self.aim.map(by);
        self.target = self.target.map(by);
        if let Some(l) = self.looking.as_mut() {
            l.cursor = by(l.cursor);
        }
        if let Some(w) = self.walking.as_mut() {
            for p in &mut w.path {
                *p = by(*p);
            }
        }
        // a ride rides on over the line, over the Net of the city it came into (built
        // there on the spot, it needn't be what the stream stood in with)
        if let Some(r) = self.riding.as_mut() {
            for p in r.path.iter_mut().chain(r.done.iter_mut()) {
                *p = by(*p);
            }
        }
        self.ride_again(st);
        // what pointed into the city you left: its foes, what was in reach, the cord's
        // walk and the floor's people
        self.last_enemy = None;
        self.which_way = None;
        self.targeting = None;
        self.placement = None;
        self.cord_walk.borrow_mut().walk_again();
        // back from the Net onto your body's cell, woken in it where its city kept it
        // (inside at the rig): the screen goes there
        if st.player.as_ref().is_some_and(|p| p.place != self.place) {
            self.follow_player(st);
        }
    }

    /// The screen's place and camera catch up with the player after a jump, and on a
    /// map that carries on past the city's edge the player looks round it.
    fn follow_player(&mut self, st: &mut AppState) {
        if let Some((place, pos)) = st.player.as_ref().map(|p| (p.place, p.pos)) {
            self.place = place;
            if carries_on(st, place) {
                look(st);
            }
            self.jump_to(st, pos);
        }
    }

    /// Puts the camera on a tile of the current place at once: snapped, clamped to
    /// that place's map (a small map sits centred; the street and the Net carry on
    /// past the city's edge, unclamped), and settled, so a new map never slides into
    /// view.
    fn jump_to(&mut self, st: &AppState, pos: Pos) {
        self.cam.snap(pos);
        if let Some(city) = st.city.as_ref() {
            let view = self.last_map.get();
            if view.width > 0 && view.height > 0 {
                if !carries_on(st, self.place) {
                    let size = TileMap::size(&self.map(st, city));
                    self.cam.clamp(size, view.width, view.height);
                }
                self.cam.settle();
            }
        }
    }

    /// The stream built under the view and round you wherever the city doesn't own the
    /// ground, before any step is checked, and the cities and towns near it building in
    /// the background; you look round again when new ground came into reach or a city
    /// came in. Off the street and the Net the builds finished still land.
    fn tend(&mut self, st: &mut AppState) {
        if !carries_on(st, self.place) {
            crate::outside::stream::collect(st);
            return;
        }
        let view = self.last_map.get();
        let origin = self.cam.origin(view.width, view.height);
        let (mut lo, mut hi) = (origin, origin.offset(view.width as i32, view.height as i32));
        let you = st.player.as_ref().filter(|p| p.place == self.place);
        let here = you.is_some();
        if let Some(p) = you {
            let r = sight_reach(st, p);
            lo = Pos::new(lo.x.min(p.pos.x - r), lo.y.min(p.pos.y - r));
            hi = Pos::new(hi.x.max(p.pos.x + r + 1), hi.y.max(p.pos.y + r + 1));
        }
        let area = TileRect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y);
        if crate::outside::stream::tend(st, area) {
            // the open Net's packets find their ways again over what came in, the cord
            // its walk and a ride its way
            self.traffic.borrow_mut().open_changed();
            self.cord_walk.borrow_mut().walk_again();
            self.ride_again(st);
            if here {
                look(st);
            }
        }
    }

    /// `>` and `<` inside: the lift panel on a lift, a floor on the stairs, nothing else;
    /// on a site's Net, the riser from a hub.
    fn vertical(&mut self, st: &mut AppState, up: bool) -> Option<ScreenCmd> {
        let player = st.player.as_ref()?;
        if let Place::SiteNet { .. } = player.place {
            let result = crate::sitenet::riser(st, up);
            report(st, result);
            return Some(ScreenCmd::Stay);
        }
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
        let rows = interact::ride_rows(st, &mut FREE_LETTERS.chars())?;
        Some(ScreenCmd::Push {
            to: Box::new(Popup::verbs("Floors", rows, st.tick)),
            fx: TransitionKind::None,
        })
    }

    /// Tiles from `pos` to the construct's entry, none when it cannot be reached.
    fn steps_to_entry(k: &citygen::Construct, pos: Pos) -> Option<usize> {
        crate::net::path_to(k, pos, k.entry).map(|p| p.len())
    }

    /// The path from the player to the aim and how many tiles the moves left cover;
    /// `None` when nothing reaches the aim. An aim off the map past the entry is
    /// the gate: the path runs to the entry and one tile out.
    /// The tiles you would walk over a stratum from `from` to `to`, after your
    /// own: the shortest way over walkable tiles with standing foes in the way.
    fn street_path(&self, st: &AppState, from: Pos, to: Pos) -> Option<Vec<Pos>> {
        let city = st.city.as_ref()?;
        let place = st.player.as_ref()?.place;
        place.stratum()?;
        if from == to {
            return Some(Vec::new());
        }
        // over the street past the city's edge too, but not across it while something
        // holds you in the city
        let beyond = Beyond::of(st);
        let map = walked_map(st, city, place, beyond.as_ref());
        let area = crate::outside::stream::path_area(city.size, from, to);
        let path = crate::outside::stream::path_within(area, from, to, |at, step| {
            (map.walkable(step)
                && crate::foes::foe_at(st, place, step).is_none()
                && crossing_refused(st, at, step).is_none())
            .then_some(1)
        })?;
        Some(path.into_iter().filter(|p| *p != from).collect())
    }

    pub fn aim_path(&self, st: &AppState) -> Option<(Vec<Pos>, usize)> {
        let aim = self.aim?;
        let player = st.player.as_ref()?;
        let Some(b) = player.place.construct() else {
            let path = self.street_path(st, player.pos, aim)?;
            let lit = match st.net.engaged.as_ref() {
                Some(e) => e.moves as usize,
                None => path.len(),
            };
            return Some((path, lit));
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

    /// One tile of an aimed walk over a stratum: a move spent, sight refreshed, the
    /// act half opening on the last move.
    fn street_walk_step(&mut self, st: &mut AppState, next: Pos) {
        let Some((place, here)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
            self.walking = None;
            return;
        };
        if let Some(why) = crossing_refused(st, here, next) {
            self.walking = None;
            st.warn(why);
            return;
        }
        let open = here.chebyshev(next) <= 1
            && open_at(st, place, next)
            && crate::foes::foe_at(st, place, next).is_none();
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
        if let Some(player) = st.player.as_mut() {
            player.pos = next;
        }
        look(st);
        stepped_outside(st, here);
        self.hand_over(st);
        let now = st.clock_tick();
        if let Some(w) = self.walking.as_mut() {
            w.path.remove(0);
            if w.path.is_empty() {
                self.walking = None;
            } else {
                w.next_at = now + crate::net::WALK_TICKS;
            }
        }
        if st.net.engaged.as_ref().is_some_and(|e| e.moves == 0) && crate::net::stay(st) {
            self.walking = None;
        }
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
        if st
            .player
            .as_ref()
            .is_some_and(|p| p.place.stratum().is_some())
        {
            self.street_walk_step(st, next);
            return;
        }
        let Some((place, b)) =
            (st.player.as_ref()).and_then(|p| Some((p.place, p.place.construct()?)))
        else {
            self.walking = None;
            return;
        };
        let here = st.player.as_ref().map(|p| p.pos);
        // The last tile off the map from the entry (off a site's gate's level) is the
        // gate: out onto the grid.
        let leaving = (st.constructs.get(b))
            .zip(here)
            .is_some_and(|(k, here)| crate::constructs::leads_out(k, here, next));
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
        // On a site's Net a door's node or a turnstile that holds ends the walk there.
        if let (Place::SiteNet { .. }, Some(me)) = (place, here)
            && let crate::venues::Meets::Held(why) = crate::sitenet::step_on_net(st, me, next)
        {
            self.walking = None;
            st.warn(why);
            return;
        }
        if let Err(why) = crate::net::spend_move(st) {
            self.walking = None;
            st.warn(why);
            return;
        }
        let bonus = st.sight_bonus();
        if let Some(city) = st.city.as_ref()
            && let Some(map) = MapRef::for_place(
                city,
                &st.interiors,
                &st.constructs,
                place,
                st.minute_of_day(),
            )
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
        // Onto a site's gate inside: out through it once every lock is broken, else the
        // walk ends on it.
        if let Place::SiteNet { building, .. } = place
            && st
                .constructs
                .site(building)
                .is_some_and(|k| k.entry == next)
        {
            self.walking = None;
            let result = crate::constructs::leave_construct(st);
            let out = result.is_ok();
            report(st, result);
            if out {
                self.follow_player(st);
                return;
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
        // Moving off closes a camera's feed.
        if after != before && st.net.feed.take().is_some() {
            st.say("feed closed");
        }
        if k.code == KeyCode::Char('.') {
            let inside = st.player.as_ref().is_some_and(|p| p.place.in_construct());
            if st.net.engaged.is_some() {
                // Only your own turn ends on the key; the others' run on the pace.
                if crate::net::your_turn(st) {
                    crate::net::end_turn(st);
                }
            } else if inside || st.net.run.is_some() {
                st.net.beat = st.clock_tick();
                crate::net::beat(st);
            }
        }
        if st.player.as_ref().map(|p| p.place) != before.map(|b| b.0) {
            self.follow_player(st);
        }
        cmd
    }

    /// A step on foot off the cells the city owns, or your Net self's into another
    /// city's or town's, onto ground you could stand on, while something holds you in the
    /// city (a fight, a call, a scene, a run in flight): refused, with the word why.
    /// `None` when the step may go on (the Net self walks off onto the open Net freely).
    fn off_the_edge(&mut self, st: &mut AppState, dir: Dir) -> Option<ScreenCmd> {
        let from = st.player.as_ref()?.pos;
        let why = crossing_refused(st, from, from.step(dir))?;
        if !open_step(st, dir) {
            return None;
        }
        st.warn(why);
        Some(ScreenCmd::Stay)
    }

    /// A step on the street or the Net off the city's map before the world out there
    /// was needed: the world made (from the city, as `outside::ensure` makes it, as
    /// leaving made it) and the ground there built, so the step can go on.
    fn world_for(&mut self, st: &mut AppState, dir: Dir) {
        let Some(p) = st.player.as_ref() else {
            return;
        };
        let out = matches!(
            p.place,
            Place::Stratum(StratumKind::Street | StratumKind::Net)
        ) && st
            .city
            .as_ref()
            .is_some_and(|c| !c.is_lot() && !c.size.contains(p.pos.step(dir)));
        if out && st.outside.is_none() {
            crate::outside::ensure(st);
            self.tend(st);
        }
    }

    fn handle_walk_inner(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        if let Some(dir) = dir_for(k.code) {
            self.world_for(st, dir);
        }
        let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
        if k.code == KeyCode::Char(' ') {
            // An aim on another tile: space confirms the walk, as enter does.
            if let Some(aim) = self.aim
                && st.player.as_ref().is_some_and(|p| p.pos != aim)
                && st.player.as_ref().is_some_and(|p| p.place.in_construct())
            {
                return Some(self.start_walk(st, aim));
            }
            // On your turn's move half, space stays put and opens the act half.
            if crate::net::stay(st) {
                self.aim = None;
                st.say("staying put · act");
                return Some(ScreenCmd::Stay);
            }
            // Off the city's cells on foot by a port of the open ground: jacked in
            // there. With none in reach, space is what it is on the street.
            if ground_port_in_reach(st) {
                jack_in_on_the_ground(st);
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
            // By another city's node jacked in, space is still this city's window (your
            // tile's Jack out and Look, the verbs held): the look cursor and a program
            // are what act on that node, and they say it's out of your body's reach.
            return self.interact(st);
        }
        // `e`: turns by hand, and back to real time when nothing hunts you.
        if k.code == KeyCode::Char('e') {
            let place = st.player.as_ref().map(|p| p.place);
            match (place, st.net.engaged.as_ref()) {
                (None, _) => st.warn("nothing to engage"),
                (Some(_), Some(e)) if e.actor() != crate::net::Actor::You => {
                    st.warn("not your turn")
                }
                (Some(_), Some(_)) if crate::net::hunting(st) => st.warn("still hunted"),
                (Some(_), Some(_)) => crate::net::disengage(st),
                (Some(place), None) if place.in_construct() => crate::net::engage(st, place, true),
                (Some(place), None) if !crate::foes::in_memory(st).is_empty() => {
                    crate::net::engage(st, place, true)
                }
                (Some(_), None) => st.warn("nothing to engage"),
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
                KeyCode::Esc | KeyCode::Char('q' | '?' | 'm' | ':' | '`' | 'x')
            )
        {
            st.warn("not your turn");
            return Some(ScreenCmd::Stay);
        }
        // `x`: look, the cursor on you.
        if k.code == KeyCode::Char('x') {
            if let Some(p) = st.player.as_ref() {
                self.which_way = None;
                self.looking = Some(Looking {
                    cursor: p.pos,
                    place: p.place,
                });
            }
            return Some(ScreenCmd::Stay);
        }
        // `J`: pulling the plug, after asking. Enter no longer jacks either way.
        if k.code == KeyCode::Char('J') {
            if !jacked {
                st.warn("not jacked in");
                return Some(ScreenCmd::Stay);
            }
            return Some(ScreenCmd::Push {
                to: Box::new(Popup::jack_out_confirm(st.tick)),
                fx: TransitionKind::None,
            });
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
        // A verb key on the Net: the verb of the held node nearest you.
        if jacked
            && let KeyCode::Char(c) = k.code
            && crate::holds::VERB_KEYS.contains(&c)
        {
            return Some(Self::verb_key(st, c));
        }
        // The aim: `g` starts it anywhere in a construct, the arrows move it while it
        // is up or while the board is on turns, enter walks it, esc drops it.
        let inside = (st.player.as_ref()).and_then(|p| Some((p.place.construct()?, p.pos)));
        // On foot on turns: `g` aims a path over the street, the arrows move the aim
        // while it is up, enter walks it; a plain arrow steps and spends a move.
        if inside.is_none()
            && crate::net::engaged_on_foot(st)
            && let Some(pos) = st.player.as_ref().map(|p| p.pos)
        {
            if k.code == KeyCode::Char('g') {
                self.aim = Some(pos);
                return Some(ScreenCmd::Stay);
            }
            if let Some(dir) = dir_for(k.code) {
                if let Some(from) = self.aim {
                    let to = from.step(dir);
                    let cap = crate::net::aim_cap(st);
                    let open = self
                        .street_path(st, pos, to)
                        .is_some_and(|p| p.len() <= cap);
                    if open {
                        self.aim = Some(to);
                    }
                    return Some(ScreenCmd::Stay);
                }
                match self.bump(st, dir) {
                    Err(cmd) => return Some(cmd),
                    Ok(true) => return Some(ScreenCmd::Stay),
                    Ok(false) => {}
                }
                return Some(self.engaged_step(st, dir));
            }
            if k.code == KeyCode::Enter
                && let Some(aim) = self.aim
                && aim != pos
            {
                return Some(self.start_walk(st, aim));
            }
        }
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
        // `W` jacked in on the city's Net: the Net over the world, to walk.
        if k.code == KeyCode::Char('W')
            && st
                .player
                .as_ref()
                .is_some_and(|p| p.jacked_in() && p.place == Place::Stratum(StratumKind::Net))
            && st.city.as_ref().is_some_and(|c| !c.is_lot())
        {
            self.up_from = st.player.as_ref().map(|p| p.pos);
            return Some(ScreenCmd::Push {
                to: Box::new(crate::screens::outside::OutsideScreen::net_from_city(st)),
                fx: TransitionKind::Glitch,
            });
        }
        // `W`: up out of the city onto the overworld, from the open street.
        if k.code == KeyCode::Char('W') {
            return Some(match crate::outside::can_leave(st) {
                Ok(()) => {
                    crate::outside::ensure(st);
                    crate::outside::moves::up(st);
                    ScreenCmd::Switch {
                        to: Box::new(crate::screens::outside::OutsideScreen::new(st)),
                        fx: TransitionKind::Glitch,
                    }
                }
                Err(why) => {
                    st.warn(why);
                    ScreenCmd::Stay
                }
            });
        }
        // A step off the city's cells on foot, while something holds you in it.
        if let Some(dir) = dir_for(k.code)
            && let Some(cmd) = self.off_the_edge(st, dir)
        {
            return Some(cmd);
        }
        if let Some(dir) = dir_for(k.code) {
            if self.start_ride(st, dir) {
                return Some(ScreenCmd::Stay);
            }
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
        let city = st.city.as_ref()?;
        let player = st.player.as_mut()?;
        match k.code {
            KeyCode::Char('c') if st.dev => {
                self.cursor = player.pos;
                self.mode = Mode::Explore;
            }
            KeyCode::Char('<') | KeyCode::Char('>') => {
                let up = k.code == KeyCode::Char('>');
                match player.use_connector(city, up) {
                    Some(dest) => {
                        self.place = Place::Stratum(dest);
                        look(st);
                        st.say(format!("now on `{}`", dest.label()));
                    }
                    None => st.warn(if up {
                        "nothing leads up from here"
                    } else {
                        "nothing leads down from here"
                    }),
                }
            }
            KeyCode::Char('.') => look(st),
            code => {
                let dir = dir_for(code)?;
                if step(st, dir) {
                    self.hand_over(st);
                } else {
                    say_why_not(st, dir);
                }
            }
        }
        Some(ScreenCmd::Stay)
    }
}

impl Screen for MapScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // A daze over: it ends.
        if st
            .missions
            .daze
            .is_some_and(|d| st.tick >= d.start + d.ticks)
        {
            st.missions.daze = None;
        }
        // A HOLO feed waiting: it opens over the map, once no scene is playing.
        if crate::missions::scene::running(st).is_none()
            && let Some(view) = st.missions.feeds.pop_front()
        {
            return ScreenCmd::Push {
                to: Box::new(crate::missions::feed::Feed::new(view, st.tick)),
                fx: TransitionKind::Glitch,
            };
        }
        // The story's tray: it opens over the map once, when no scene is playing; esc
        // leaves it waiting for you to talk to whoever holds it.
        if crate::missions::scene::running(st).is_none()
            && let Some(tray) = st.missions.tray.as_mut().filter(|t| !t.shown)
        {
            tray.shown = true;
            if let Some(popup) = Popup::tray(st) {
                return ScreenCmd::Push {
                    to: Box::new(popup),
                    fx: TransitionKind::None,
                };
            }
        }
        // Black ICE killed: the run is marked if it was your only one, and the screen
        // reads the cause and how it ends. The track ends with you.
        if let Some(cause) = st.net.death.clone() {
            st.track = None;
            let end = crate::screens::flatlined::end(st, &cause);
            return ScreenCmd::Switch {
                to: Box::new(crate::screens::flatlined::Flatlined::new(
                    cause,
                    &st.character.handle,
                    st.character.level,
                    st.tick,
                    end,
                )),
                fx: TransitionKind::Glitch,
            };
        }
        // The console's `target`: the free camera on the tile, the inspector reading it.
        if let Some(pos) = st.jump_to.take() {
            st.dev = true;
            self.mode = Mode::Explore;
            self.place = Place::Stratum(StratumKind::Street);
            self.cursor = pos;
            self.cam.snap(pos);
            st.sidebar.set(Section::Inspect, true);
            st.repaint = true;
        }
        // A popup's Look row: the cursor goes on its tile.
        if let Some(pos) = st.look_at.take()
            && self.mode == Mode::Walk
        {
            self.which_way = None;
            self.looking = Some(Looking {
                cursor: pos,
                place: self.place,
            });
        }
        // A level plays its upload on you once you're walking with nothing over the
        // map: no scene, no rest's timeline, no console. Until then it waits.
        let clear = crate::missions::scene::running(st).is_none()
            && st.rest.is_none()
            && self.console.is_none()
            && self.mode == Mode::Walk;
        if clear
            && let Some((place, origin)) = st.player.as_ref().map(|p| (p.place, p.pos))
            && let Some(level) = st.levelled.take()
        {
            st.fx.push(crate::app::FxInstance {
                fx: game::Fx::Upload,
                place,
                origin,
                radius: 0,
                started: st.tick,
                revealed: Vec::new(),
                targets: Vec::new(),
                label: format!("LEVEL {level}"),
                hit: false,
                tint: None,
                arc: None,
            });
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
        // The map follows the player: a popup may have jacked them or taken the stairs,
        // or the Net over the world dropped them onto another tile's Net.
        let up_from = self.up_from.take();
        if self.mode == Mode::Walk
            && st
                .player
                .as_ref()
                .is_some_and(|p| p.place != self.place || up_from.is_some_and(|q| q != p.pos))
        {
            self.follow_player(st);
        }
        // Put on a cell another city or town owns (a run opened there, a landing): into
        // it, as a step onto it goes in.
        if self.mode == Mode::Walk {
            self.hand_over(st);
        }
        // The open ground under the view, before any step is checked.
        self.tend(st);
        // The console takes every key while it is open.
        let focus = game::At {
            place: self.place,
            pos: match (self.mode, &st.player) {
                (Mode::Walk, Some(p)) => p.pos,
                _ => self.cursor,
            },
        };
        if self.console.is_some() {
            return self
                .feed_console(st, focus, keys)
                .unwrap_or(ScreenCmd::Stay);
        }
        let Some(size) = st.city.as_ref().map(|c| c.size) else {
            if keys
                .iter()
                .any(|k| matches!(k.code, KeyCode::Char('q') | KeyCode::Esc))
            {
                return ScreenCmd::Switch {
                    to: Box::new(Title::new(st, st.tick)),
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
        // A scene took a key this frame.
        let mut scened = false;
        for (i, k) in keys.iter().enumerate() {
            // A ride in flight: any key stops it where it is, and does nothing else,
            // before a scene, a rest or a call could take it.
            if self.riding.take().is_some() {
                continue;
            }
            // Once a scene has had a key, the rest of the frame's are its too, or
            // nobody's: a second `esc` as one scene ends and the next waits its tick
            // never reaches the map.
            if scened {
                crate::missions::scene::key(st, k);
                continue;
            }
            // A scene playing takes every key, all but the `:` that opens the dev
            // console over it.
            let console = st.dev && k.code == KeyCode::Char(':');
            if !console && crate::missions::scene::key(st, k) {
                scened = true;
                continue;
            }
            // A rest takes every key but esc, which wakes you (and the console's, once
            // open, its own).
            if !console && self.console.is_none() && crate::rest::key(st, k) {
                continue;
            }
            // A HOLO call up takes space and esc before the walk keys, once look mode,
            // a pick, an aim and help have had theirs.
            if self.plain_walk() && crate::missions::call::key(st, k) {
                continue;
            }
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
                // So does look mode.
                if self.looking.is_some() {
                    let cmd = self.handle_looking(st, k);
                    if !matches!(cmd, ScreenCmd::Stay) {
                        return cmd;
                    }
                    continue;
                }
                // Your act half on foot: the cursor takes its keys, the rest go on.
                if self.firing(st)
                    && crate::net::your_turn(st)
                    && let Some(cmd) = self.handle_firing(st, k)
                {
                    if !matches!(cmd, ScreenCmd::Stay) {
                        return cmd;
                    }
                    continue;
                }
                // So does the which-way pick.
                if self.which_way.is_some() {
                    let cmd = self.handle_which_way(st, k);
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
                return self
                    .feed_console(st, focus, &keys[i + 1..])
                    .unwrap_or(ScreenCmd::Stay);
            }
        }
        // A key may have taken the player through a door, up a lift or out again: the
        // map follows before this frame draws.
        if self.mode == Mode::Walk && st.player.as_ref().is_some_and(|p| p.place != self.place) {
            self.follow_player(st);
        }
        if !st.dev && self.mode == Mode::Explore {
            // Without dev mode there is no free camera: the player is always on foot.
            self.enter_walk(st);
        }
        let map = self.last_map.get();
        match (self.mode, &st.player, crate::missions::scene::camera(st)) {
            // A scene holding a tile of this map: the camera centres on it.
            (Mode::Walk, _, Some((place, pos))) if place == self.place => self.cam.target(pos),
            (Mode::Walk, Some(p), _) => self.cam.follow(p.pos, map.width, map.height),
            _ => self.cam.follow(self.cursor, map.width, map.height),
        }
        // The camera is bound by the map it shows, not the city: a floor or a construct
        // smaller than the view sits centred. The street and the Net carry on past the
        // city's edge, and the camera with them.
        if !carries_on(st, self.place) {
            let bounds = st
                .city
                .as_ref()
                .map(|c| TileMap::size(&self.map(st, c)))
                .unwrap_or(size);
            self.cam.clamp(bounds, map.width, map.height);
        }
        self.cam.update();
        // The city lives: a minute step when the clock turns, a few paths per frame,
        // and nobody walks onto the player.
        let standing = match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) if p.place.is_street() => Some(p.pos),
            _ => None,
        };
        if let (Some(_), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.set_blocker(standing);
        }
        let inside = st.player.as_ref().and_then(|p| p.place.construct());
        let before = st.player.as_ref().map(|p| (p.place, p.pos));
        let resting = st.rest.is_some();
        if resting {
            crate::rest::frame(st);
        } else {
            crate::clock::run(
                st,
                (self.mode == Mode::Walk).then_some(elapsed),
                crate::clock::PATHS_PER_FRAME,
            );
        }
        // The mails landed since the map's last frame, here or under a screen on top.
        crate::mail_mark::notice(st);
        // The story moved you (an `Arrive`, a scene's walk): no aimed walk goes on from
        // where you were, and the map follows you before this frame draws.
        if st.player.as_ref().map(|p| (p.place, p.pos)) != before {
            self.walking = None;
            self.riding = None;
            self.aim = None;
            if self.mode == Mode::Walk && st.player.as_ref().is_some_and(|p| p.place != self.place)
            {
                self.follow_player(st);
            }
        }
        // The cities and towns next door in view live on the same clock, run once the map
        // has followed you, so none is drawn behind it in the view this frame shows.
        if !resting && self.mode == Mode::Walk && carries_on(st, self.place) {
            let origin = self.cam.origin(map.width, map.height);
            let view = TileRect::new(origin.x, origin.y, map.width as i32, map.height as i32);
            crate::outside::stream::run_neighbours(
                st,
                view,
                crate::clock::PATHS_PER_FRAME,
                elapsed,
            );
        }
        // A scene playing or a rest under way: no aimed walk goes on under it, and
        // nothing of the map's stays open to keep its keys after.
        if crate::missions::scene::running(st).is_some() || st.rest.is_some() {
            self.walking = None;
            self.riding = None;
            self.aim = None;
            self.help = false;
            self.looking = None;
            self.which_way = None;
            self.targeting = None;
        }
        crate::track::tick(st);
        // Beside a venue's door or on its crawl's tile: its plan kept, for Space and Look.
        crate::venues::keep_plans_here(st);
        // The asides run out on the clock, and all go once a scene, a rest or a fight is on.
        crate::missions::asides::prune(st);
        crate::missions::news::tick(st, self.mode == Mode::Walk);
        crate::missions::ad::tick(st, self.mode == Mode::Walk);
        // A fixer beside you has their offer ready before you talk.
        if self.mode == Mode::Walk {
            crate::jobs::ui::prepare_offers(st, &|pl, p| self.inside_person_at(pl, p));
        }
        // A thrown live blast that came down this frame is heard before anyone looks.
        crate::blast::land(st);
        // The beat of a calm construct: one per frame at most, so a long menu is
        // never a burst of rolls.
        let was_engaged = st.net.engaged.is_some();
        // Carried to another map, out of the fight.
        if let Some(e) = st.net.engaged.as_ref()
            && st.player.as_ref().is_some_and(|p| p.place != e.place)
        {
            crate::net::disengage(st);
        }
        if self.mode == Mode::Walk && inside.is_some() {
            if st.net.engaged.is_some() {
                crate::net::pace(st);
            } else if st.clock_tick() >= st.net.beat + crate::net::BEAT_TICKS {
                st.net.beat = st.clock_tick();
                crate::net::beat(st);
            }
        }
        // On foot: a foe that sees you puts the map on turns; while it is, they act
        // on the pace.
        if self.mode == Mode::Walk
            && inside.is_none()
            && let Some(place) = st.player.as_ref().map(|p| p.place)
        {
            if st.net.engaged.is_some() {
                crate::net::pace(st);
            } else {
                // Calm: a fuse burns down on every beat; on the open Net, a run in
                // flight rolls on it.
                let rolling = st.net.run.is_some() && place == Place::Stratum(StratumKind::Net);
                if (!st.live.is_empty() || rolling)
                    && st.clock_tick() >= st.net.beat + crate::net::BEAT_TICKS
                {
                    st.net.beat = st.clock_tick();
                    crate::net::beat(st);
                }
                if st.net.engaged.is_none() && crate::foes::look_all(st) {
                    crate::net::engage(st, place, false);
                }
            }
        }
        // Your last action point gone: the turn passes on its own a beat later.
        if self.mode == Mode::Walk {
            crate::net::auto_end(st);
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
            self.targeting = None;
            if !crate::net::engaged_on_foot(st) {
                self.aim = None;
                self.walking = None;
            }
        }
        // The turn left you while you were picking: nothing to confirm now.
        if self.targeting.is_some() && !crate::net::your_turn(st) {
            self.targeting = None;
        }
        // Carried to another map: the look ends.
        if self
            .looking
            .is_some_and(|l| st.player.as_ref().map(|p| p.place) != Some(l.place))
        {
            self.looking = None;
        }
        // Moved off, or carried away: the pick no longer points anywhere.
        if self
            .which_way
            .as_ref()
            .is_some_and(|w| st.player.as_ref().map(|p| (p.place, p.pos)) != Some(w.from))
        {
            self.which_way = None;
        }
        if self.mode == Mode::Walk {
            self.walk_step(st);
            self.ride_step(st);
        }
        // The fire cursor: opens on the last or nearest enemy as your act half
        // opens (a key, or the walk's last move), goes when the half does.
        let phase = crate::net::phase(st);
        if phase == Some(crate::net::Phase::Act) {
            if self.last_phase != Some(crate::net::Phase::Act) {
                self.open_target(st);
            }
        } else {
            self.target = None;
            self.armed = None;
        }
        self.last_phase = phase;
        self.refresh_placement(st);
        // A save asked for, or five minutes of play since the last.
        crate::runs::when::frame(st);
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        st.drawn_you.set(None);
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
        let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
        let (route, home_trace) = self.route_parts(st, jacked);
        let r = layout(area, !route.is_empty() || home_trace.is_some());
        // A film scene frames the map in the middle of a black frame.
        let film = crate::missions::scene::running(st)
            .is_some_and(|s| s.look() == crate::missions::Look::Film);
        let map_area = if film {
            crate::missions::film::map_rect(area, r.map)
        } else {
            r.map
        };
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
        let walking =
            self.mode == Mode::Walk && st.player.as_ref().is_some_and(|p| p.place == self.place);
        let vis = if walking {
            st.player.as_ref().map(|p| p.vis())
        } else {
            None
        };
        // The street and the Net carry on past the city's edge: the areas there are
        // drawn first, the city then only on the cells it owns.
        let beyond = matches!(
            self.place,
            Place::Stratum(StratumKind::Street | StratumKind::Net)
        )
        .then(|| Beyond::of(st))
        .flatten();
        let live_owns = |p: Pos| beyond.as_ref().is_none_or(|b| b.live_owns(p));
        let keep = beyond.as_ref().map(|_| &live_owns as &dyn Fn(Pos) -> bool);
        // The look-down: the ground plaza under an upper level's atrium, drawn
        // before anything else so the current level stays on top.
        let atrium_floor = match (self.place, &map) {
            (Place::Floor { building, level }, MapRef::Floor(m))
                if level >= 1 && m.plan.atrium.is_some() =>
            {
                st.interiors
                    .floor(building, 0)
                    .map(|(_, ground)| (ground, level, m.floor.tiles.size()))
            }
            _ => None,
        };
        // Looking around without walking sees the whole map, the drop included.
        let open_sight = match (atrium_floor, vis) {
            (Some((_, _, size)), None) => Some(citygen::Grid::filled(
                size,
                crate::render::Visibility::Visible,
            )),
            _ => None,
        };
        let atrium = atrium_floor.and_then(|(ground, level, _)| {
            Some(AtriumOverlay {
                ground,
                level,
                vis: vis.or(open_sight.as_ref())?,
            })
        });
        let mut overlays: Vec<&dyn Overlay> = Vec::new();
        if let Some(o) = atrium.as_ref() {
            overlays.push(o);
        }
        // The city's Net's traffic moves on with the clock round the view.
        let (packets, soaks) = if self.place == Place::Stratum(StratumKind::Net) {
            let view = citygen::Rect::new(
                origin.x,
                origin.y,
                map_area.width as i32,
                map_area.height as i32,
            );
            self.traffic
                .borrow_mut()
                .city(city.seed)
                .run_city(city, view, st.tick)
        } else {
            Default::default()
        };
        // The Net's own look, under everything else drawn on it.
        let net_view = crate::render::netlook::NetLookView {
            tick: st.tick,
            packets,
            soaks,
            tracked: match crate::track::target_now(st) {
                Some(crate::track::Target::Building(b)) => Some(b),
                _ => None,
            },
            home: st.home.map(|h| h.home.building),
            watch: match self.place {
                Place::Construct(b) | Place::SiteNet { building: b, .. } => st
                    .constructs
                    .get(b)
                    .map(crate::net::watched)
                    .unwrap_or_default(),
                _ => Default::default(),
            },
        };
        let net_look = crate::render::netlook::NetLook {
            view: &net_view,
            keep,
        };
        if self.place == Place::Stratum(StratumKind::Net) || self.place.in_construct() {
            overlays.push(&net_look);
        }
        // The cord: the way back to where you jacked in (Settings can hide it), walked
        // as far as the view shows it.
        let shown = TileRect::new(
            origin.x,
            origin.y,
            map_area.width as i32,
            map_area.height as i32,
        );
        let cord = crate::net::cord_view(st)
            .filter(|_| self.place == Place::Stratum(StratumKind::Net))
            .zip(st.player.as_ref().map(|p| p.pos))
            .map(|(mut view, you)| {
                view.walk = self.cord_walk.borrow_mut().walk(st, view.entry, you, shown);
                view
            });
        let walked = walked_map(st, city, self.place, beyond.as_ref());
        let cord_overlay = cord.as_ref().map(|view| crate::render::cord::CordOverlay {
            view,
            walked: &walked,
        });
        if let Some(o) = cord_overlay.as_ref() {
            overlays.push(o);
        }
        let ride = self
            .riding
            .as_ref()
            .zip(st.player.as_ref().filter(|p| p.place == self.place))
            .map(|(ride, p)| crate::render::ride::RideOverlay { ride, me: p.pos });
        if let Some(o) = ride.as_ref() {
            overlays.push(o);
        }
        let weather = Weather { keep };
        if st.rain {
            overlays.push(&weather);
        }
        overlays.push(&digital);
        // The venues' doors: a mission's flagged, and those onto floors the plan lacks shut.
        let marks = if self.place.stratum().is_some() {
            crate::venues::door_marks(st)
        } else {
            Vec::new()
        };
        let door_marks = crate::render::doors::DoorMarks { marks: &marks, vis };
        if !marks.is_empty() {
            overlays.push(&door_marks);
        }
        // A site's floor: its terminals and gate switches on their wall tiles, where you
        // jack in.
        let devices = self.floor_devices(st);
        let floor_devices = crate::render::doors::FloorDevices {
            devices: &devices,
            vis,
        };
        if !devices.is_empty() {
            overlays.push(&floor_devices);
        }
        // On the ground, under whatever lies or stands on it: the scorch a blast left
        // and a live blast's shading.
        let scorch = ScorchOverlay { fx: &st.fx, vis };
        overlays.push(&scorch);
        // On the street or a site's floor: the cones of cameras that see you, or that you
        // hold.
        let cones = match self.place {
            Place::Stratum(StratumKind::Street) | Place::Floor { .. } => {
                crate::cameras::cones(st, self.place)
            }
            _ => Vec::new(),
        };
        let cone_overlay = crate::cameras::ConeOverlay { cones: &cones, vis };
        if !cones.is_empty() {
            overlays.push(&cone_overlay);
        }
        let live_place = st.player.as_ref().map(|p| p.place);
        let live_ground = live_place
            .map(|pl| crate::blast::live_footprints(st, pl))
            .unwrap_or_default();
        let live_shade = crate::render::path::LiveGround {
            footprints: &live_ground,
        };
        if !live_ground.is_empty() {
            overlays.push(&live_shade);
        }
        let ground = GroundOverlay {
            ground: &st.ground,
            content: &st.content,
            vis,
        };
        overlays.push(&ground);
        let (hour, _) = st.clock();
        let eyes = if st.show_eyes {
            crate::exposure::of(st).eyes
        } else {
            Vec::new()
        };
        let people = st.world.as_ref().map(|world| PeopleOverlay {
            world,
            night: hour >= 18 || hour <= 5,
            vis,
            eyes: &eyes,
            foes: &st.foes,
            bodies: &st.missions.bodies,
            keep,
        });
        if let Some(p) = people.as_ref() {
            overlays.push(p);
        }
        // Your bots over the walkers, under the aim and your @.
        let bots = crate::render::bots::BotOverlay {
            bots: &st.bots,
            content: &st.content,
            foes: &st.foes,
            vis,
            beyond: st.player.as_ref().map_or(&[], |p| p.beyond_sight()),
            now: st.clock_tick(),
        };
        overlays.push(&bots);
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
            Place::Construct(b) | Place::SiteNet { building: b, .. } => {
                Some(crate::render::ghost::GhostOverlay {
                    steps: crate::net::ghost_steps(st, b),
                })
            }
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
        let ways: Vec<Pos> = self
            .which_way
            .as_ref()
            .map(|w| w.subjects.iter().map(|s| s.pos).collect())
            .unwrap_or_default();
        let way_blocks = crate::render::path::BlockOverlay {
            tiles: &ways,
            bg: crate::render::path::WAY_BG,
        };
        if !ways.is_empty() {
            overlays.push(&way_blocks);
        }
        // The fire cursor: the line to it, the foe under it inverted, the readout.
        let fire_line: Vec<Pos> = match (self.target, st.player.as_ref()) {
            // Whoever stands on the way is drawn over the tether, not under it.
            (Some(t), Some(p)) if self.firing(st) => citygen::pathing::bresenham(p.pos, t)
                .into_iter()
                .filter(|q| {
                    *q != p.pos && *q != t && crate::foes::foe_at(st, self.place, *q).is_none()
                })
                .collect(),
            _ => Vec::new(),
        };
        let aimed_foe = self
            .target
            .filter(|_| self.firing(st))
            .and_then(|t| Self::enemy_at(st, t));
        // The readout steps aside while a shot's pictures play on the target.
        let shooting = self.target.is_some_and(|t| {
            st.fx.iter().any(|i| {
                i.place == self.place
                    && i.fx.is_shot()
                    && st.tick < i.until()
                    && i.targets
                        .first()
                        .is_some_and(|p| p.y == t.y && (p.x - t.x).abs() <= 2)
            })
        });
        let armed = self
            .armed
            .as_ref()
            .zip(self.target)
            .filter(|_| self.firing(st));
        let armed_text = armed.and_then(|(id, t)| Self::armed_readout(st, id, t));
        let footprint = armed
            .map(|(id, t)| Self::armed_footprint(st, id, t))
            .unwrap_or_default();
        let you = st
            .player
            .as_ref()
            .map(|p| p.pos)
            .filter(|me| footprint.iter().any(|(q, _)| q == me));
        let (readout, lit) = match &armed_text {
            Some((text, _)) => (Some(text.clone()), None),
            None => match aimed_foe.filter(|_| !shooting) {
                Some(i) => {
                    let (text, lit) = Self::aim_readout(st, i);
                    (Some(text), lit)
                }
                None => (None, None),
            },
        };
        let fire =
            self.target
                .filter(|_| self.firing(st))
                .map(|t| crate::render::path::AimOverlay {
                    line: &fire_line,
                    target: t,
                    on_foe: aimed_foe.is_some(),
                    readout,
                    footprint: &footprint,
                    you,
                    warn: armed_text.as_ref().is_some_and(|(_, warn)| *warn),
                    lit,
                });
        // A live blast's thing under a foe standing on it: the foe's glyph is kept.
        let drawn_foes: Vec<Pos> = st
            .foes
            .iter()
            .filter(|f| f.standing() && f.place == self.place)
            .filter(|f| {
                vis.is_none_or(|v| v.get(f.pos) == Some(&crate::render::Visibility::Visible))
            })
            .map(|f| f.pos)
            .collect();
        let live = crate::render::path::LiveOverlay {
            live: &st.live,
            standing: &drawn_foes,
        };
        if !st.live.is_empty() {
            overlays.push(&live);
        }
        // After the live blasts: the aim draws over them.
        if let Some(o) = fire.as_ref() {
            overlays.push(o);
        }
        let looked: Vec<Pos> = self.looking.iter().map(|l| l.cursor).collect();
        let look_block = crate::render::path::BlockOverlay {
            tiles: &looked,
            bg: crate::look::LOOK_BG
                .scale(0.8 + 0.4 * crate::anim::effects::pulse(st.tick, 30))
                .to_color(),
        };
        if !looked.is_empty() {
            overlays.push(&look_block);
        }
        // The turn column, on the map that is on turns.
        let turns = st
            .net
            .engaged
            .as_ref()
            .filter(|e| e.place == self.place)
            .and_then(|_| crate::net::turn_view(st));
        let turn_overlay = turns
            .as_ref()
            .map(|view| crate::render::turns::TurnOverlay { view });
        // A calm run hangs off its target, under a roll's bolt and a slip's mark; when
        // it has no room the footer reads it instead.
        // Sweeps ringing out of alarmed hubs on the open Net.
        let rings = crate::grid::RingOverlay {
            rings: &st.net.grid.rings,
            tick: st.clock_tick(),
        };
        if self.place == Place::Stratum(StratumKind::Net) && !st.net.grid.rings.is_empty() {
            overlays.push(&rings);
        }
        // A decoy floods the ad board's room.
        let flood = match self.place {
            Place::Construct(b) | Place::SiteNet { building: b, .. } => {
                crate::holds::decoy_room(st, b)
            }
            _ => Vec::new(),
        };
        let flood_blocks = crate::render::path::BlockOverlay {
            tiles: &flood,
            bg: crate::holds::FLOOD_BG,
        };
        if !flood.is_empty() {
            overlays.push(&flood_blocks);
        }
        // Every node you hold is drawn green; the nearest within reach also carries
        // the dock, when no run is in flight.
        let held_tiles: Vec<Pos> = crate::holds::held_on(st, self.place)
            .into_iter()
            .map(|(_, at)| at)
            .collect();
        let held = crate::holds::HeldOverlay { tiles: &held_tiles };
        if !held_tiles.is_empty() {
            overlays.push(&held);
        }
        let tether = crate::net::tether_view(st, self.place)
            .or_else(|| crate::holds::dock_view(st, self.place))
            .and_then(|view| {
                let layout = crate::render::tether::layout(&view, map_area, origin, |p| {
                    map.tile(p).is_some()
                })?;
                Some((view, layout))
            });
        let tether_overlay =
            tether
                .as_ref()
                .map(|(view, layout)| crate::render::tether::TetherOverlay {
                    view,
                    layout: *layout,
                });
        if let Some(o) = tether_overlay.as_ref() {
            overlays.push(o);
        }
        // The live shapes draw over everything on the map; their scorch is on the
        // ground layer above.
        let clear_right = turns.as_ref().map_or(0, |_| crate::render::turns::WIDTH);
        let fx = FxOverlay {
            fx: &st.fx,
            vis,
            clear_right,
        };
        overlays.push(&fx);
        let light = crate::render::lighting::LightOverlay { vis, keep };
        if st.show_light {
            overlays.push(&light);
        }
        if let Some(o) = turn_overlay.as_ref() {
            overlays.push(o);
        }
        // The tracked walk, last: it only tints backgrounds, and only on its own place.
        // A film shows none of the HUD, the walk and its edge tag neither.
        let tracked_leg = crate::track::leg_now(st)
            .and_then(Result::ok)
            .filter(|l| !film && l.place == self.place);
        let track_overlay = tracked_leg.map(|leg| crate::track::overlay::TrackOverlay {
            leg,
            pulses: crate::track::pulses_now(st),
            arrive: leg.through == crate::track::route::Through::Arrive,
        });
        if let Some(o) = track_overlay.as_ref() {
            overlays.push(o);
        }
        // Holo ads hijacked in the cities held as you left them, in sight past the city's
        // cells: they glow as they do from inside.
        let mut held_ads = Vec::new();
        if let (Some(b), Some(stratum)) = (beyond.as_ref(), self.place.stratum()) {
            // Off the city's map: in sight now, else what the world's record remembers;
            // on it, your memory of the city's cells (another area's among them), as
            // Look sees them. Looking round without walking sees it all.
            let past = |p: Pos| match vis {
                Some(_) => crate::look::sight(st, Some(b), self.place, p),
                None => crate::render::Visibility::Visible,
            };
            let mut nets = self.traffic.borrow_mut();
            draw_beyond(
                st,
                b,
                stratum,
                origin,
                map_area,
                &past,
                &mut nets,
                f.buffer_mut(),
            );
            if self.place.is_street() {
                held_ads = hijacked_beyond(st, b, &past);
            }
        }
        MapView {
            map: &map,
            origin,
            tick: st.tick,
            style: &st.style,
            vis,
            keep,
            overlays: &overlays,
            cursor: if self.mode == Mode::Explore {
                Some(cursor)
            } else {
                None
            },
        }
        .render(map_area, f.buffer_mut());
        // The flat's lights and the story's visitors, under your own `@`.
        if st.player.as_ref().is_some_and(|p| p.place == self.place) {
            crate::missions::visitors::draw(f.buffer_mut(), map_area, origin, st);
        }
        // Your flat's holo set, playing the news while it is on: in sight, not through
        // a wall.
        if crate::missions::news::current(st).is_some_and(|n| n.on)
            && let Some((place, pos)) = crate::missions::news::set_tile(st)
            && place == self.place
            && vis.is_none_or(|v| v.get(pos) == Some(&crate::render::Visibility::Visible))
            && let Some((sx, sy)) = tile_to_screen(map_area, origin, pos)
        {
            f.buffer_mut()[(sx, sy)]
                .set_char(crate::render::tileset::holo_glyph(&st.style, st.tick))
                .set_fg(crate::ui::chrome::MAGENTA);
        }
        // A hijacked building's holo ads glow magenta, glyph kept: the live city's, and
        // those of the cities held as you left them.
        if self.place.is_street() {
            for (pos, _) in crate::holds::hijacked_ads(st) {
                if vis.is_none_or(|v| v.get(pos) == Some(&crate::render::Visibility::Visible))
                    && let Some((sx, sy)) = tile_to_screen(map_area, origin, pos)
                {
                    f.buffer_mut()[(sx, sy)].set_fg(crate::ui::chrome::MAGENTA);
                }
            }
            for pos in held_ads {
                if let Some((sx, sy)) = tile_to_screen(map_area, origin, pos) {
                    f.buffer_mut()[(sx, sy)].set_fg(crate::ui::chrome::MAGENTA);
                }
            }
        }
        // A holo ad cutting into the news, or torn by a story's line: its glow in MAGENTA,
        // its glyph kept.
        if let Some(pos) =
            crate::missions::news::cut_in(st).or_else(|| crate::missions::ad::cut_in(st))
            && self.place.is_street()
            && vis.is_none_or(|v| v.get(pos) == Some(&crate::render::Visibility::Visible))
            && let Some((sx, sy)) = tile_to_screen(map_area, origin, pos)
        {
            f.buffer_mut()[(sx, sy)].set_fg(crate::ui::chrome::MAGENTA);
        }

        if let (Some(leg), Some(m)) = (tracked_leg, crate::track::distance(st)) {
            let label = st.track.as_ref().map_or("", |t| t.label.as_str());
            let text =
                crate::track::overlay::tag_text(label, crate::track::next_now(st), leg.through, m);
            crate::track::overlay::draw_edge_tag(
                f.buffer_mut(),
                map_area,
                origin,
                leg,
                &text,
                st.tick,
            );
        }

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
                // On the Net and in a construct you are blue on a blue glow, apart
                // from the greens and cyans around you; elsewhere white.
                let style = if p.sees_all() {
                    Style::default()
                        .fg(crate::render::netlook::YOU.to_color())
                        .bg(crate::render::netlook::YOU_GLOW.to_color())
                } else {
                    Style::default().fg(Color::Rgb(255, 255, 255))
                };
                f.buffer_mut()[(sx, sy)]
                    .set_char('@')
                    .set_style(style.add_modifier(Modifier::BOLD));
            }
        }
        // The daze swims the whole map picture, `@` and visitors with it; what floats
        // over the map below stays still.
        crate::missions::daze::draw(f.buffer_mut(), map_area, st);
        // A tapped camera's feed hangs off its node, away from the dock's label.
        if st.net.feed.is_some() && self.place == st.player.as_ref().map_or(self.place, |p| p.place)
        {
            let feed_at = st
                .net
                .feed
                .and_then(|n| crate::holds::node_pos(st, self.place, n));
            let avoid = tether
                .as_ref()
                .filter(|(view, _)| Some(view.target) == feed_at)
                .map(|(_, lay)| lay.left);
            crate::render::feed::draw(
                f.buffer_mut(),
                map_area,
                origin,
                st,
                avoid,
                area.width >= 110,
            );
        }
        // Your `@`'s cell, kept for what draws over the map: the HOLO call, the Rest
        // window here and the one a screen on top draws.
        let me = st
            .player
            .as_ref()
            .filter(|p| p.place == self.place)
            .and_then(|p| tile_to_screen(map_area, origin, p.pos));
        st.drawn_you.set(me);
        if film {
            self.draw_film(st, f, map_area, origin);
            return;
        }
        // A bubbles scene keeps the frame; its doors and marker go on the map.
        let bubbles = crate::missions::scene::running(st)
            .is_some_and(|s| s.look() == crate::missions::Look::Bubbles);
        if bubbles {
            crate::missions::overlay::draw(f.buffer_mut(), map_area, origin, st);
        }
        if let Some(rest) = &st.rest {
            if rest.kind == crate::rest::Kind::Sleep
                && let Some(me) = me
            {
                crate::rest::draw_zs(f.buffer_mut(), map_area, me);
            }
            crate::rest::draw_window(
                f.buffer_mut(),
                map_area,
                me,
                st,
                rest.kind,
                rest.until,
                Some(rest.from),
            );
        }
        // Asides over the map, off your `@`: only on foot with the console shut (and, as
        // `draw` sees to, no scene, rest or fight).
        if self.mode == Mode::Walk && self.console.is_none() {
            crate::missions::asides::draw(f.buffer_mut(), map_area, origin, st);
        }
        // A mail just landed: the pill on the map's top edge, right, a row in; held while
        // a call or a rest covers the map's top.
        let inset = crate::mail_mark::PILL_INSET;
        let pill = crate::mail_mark::pill_line(st, map_area.width.saturating_sub(inset))
            .filter(|p| p.width() > 0 && map_area.height > 1);
        if let Some(line) = &pill {
            let w = line.width() as u16;
            f.render_widget(
                Paragraph::new(line.clone()),
                Rect::new(map_area.right() - inset - w, map_area.y + 1, w, 1),
            );
        }
        // The HOLO call in the map's corner, kept off your `@`.
        crate::missions::call::draw(f.buffer_mut(), map_area, me, st);

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
                // With no pill drawn (down, or held), the unread mail on the status line.
                mail: if pill.is_some() {
                    0
                } else {
                    crate::mail_mark::unread(st)
                },
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
        // On foot or jacked in off the city's cells, where you are as HERE names it.
        let mut location = match st.player.as_ref() {
            Some(p) if self.mode == Mode::Walk && p.place == self.place && !live_owns(p.pos) => {
                crate::outside::here::band(st, self.place == Place::Stratum(StratumKind::Net))
            }
            _ => None,
        }
        .unwrap_or_else(|| crate::look::location(st, self.place, cursor));
        let alert_building = match self.place {
            Place::Construct(b)
            | Place::SiteNet { building: b, .. }
            | Place::Floor { building: b, .. } => Some(b),
            Place::Stratum(_) => None,
        };
        if alert_building.is_some_and(|b| st.net.alert(b, st.net.now(st))) {
            location = format!("ALERT · {location}");
        }
        f.render_widget(
            Paragraph::new(hud::location_line(&location, st.clock(), rw)),
            r.band_r[0],
        );
        let exposure =
            if self.place == Place::Stratum(StratumKind::Net) || self.place.in_construct() {
                crate::exposure::net_text(self.place, crate::net::hunted(st))
            } else if matches!(self.place, Place::Stratum(_)) {
                crate::exposure::text(&crate::exposure::of(st))
            } else {
                String::new()
            };
        f.render_widget(
            Paragraph::new(hud::exposure_row(&exposure, rw)),
            r.band_r[2],
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
        // Right column. The dev inspector follows the look cursor; HERE stays on you.
        let inspect = self.looking.map_or(cursor, |l| l.cursor);
        let person = st
            .world
            .as_ref()
            .filter(|_| self.place.is_street())
            .and_then(|w| w.person_at(inspect).map(|id| w.card(id, city)))
            .or_else(|| {
                let id = self.inside_person_at(self.place, inspect)?;
                st.world.as_ref().map(|w| w.card(id, city))
            });
        let view = sidebar::SidebarView {
            map,
            focus: cursor,
            inspect,
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
            track: crate::track::mini::map_track(st, self.place),
            style: &st.style,
            content: &st.content,
            ground: &st.ground,
            log: &st.log,
            tick: st.tick,
            dev: st.dev,
            net: &st.net,
            now: st.net.now(st),
            jobs: crate::missions::ui::jobs_pane(st),
            story_lines: crate::missions::ui::story_line_count(st),
            card: match (self.place, st.player.as_ref()) {
                (Place::Construct(b) | Place::SiteNet { building: b, .. }, Some(p))
                    if p.place == self.place =>
                {
                    crate::net::card_lines(st, b, None)
                }
                (Place::Stratum(StratumKind::Net), Some(p)) if p.place == self.place => st
                    .net
                    .nearest_trace(city, p.pos)
                    .map(|(b, _)| crate::net::card_lines(st, b, Some(p.pos)))
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            tracked: match crate::track::target_now(st) {
                Some(crate::track::Target::Building(b)) => Some(b),
                _ => None,
            },
            port: crate::net::port_of(st),
            outside: match st.player.as_ref() {
                Some(p)
                    if p.place == self.place
                        && beyond.as_ref().is_some_and(|b| !b.live_owns(p.pos)) =>
                {
                    match self.place {
                        Place::Stratum(StratumKind::Net) => {
                            crate::outside::here::net_here_lines(st)
                        }
                        _ => {
                            let mut l = crate::outside::here::here_lines(st);
                            l.extend(crate::outside::here::near_lines(st, NEAR_ROWS));
                            l
                        }
                    }
                }
                _ => Vec::new(),
            },
            beyond: beyond.as_ref(),
            ice_lines: match (self.place, st.player.as_ref()) {
                (place, Some(p)) if place.in_construct() && p.place == place => {
                    let vis = p.vis();
                    crate::net::ice_lines(st, place, |q| {
                        vis.get(q)
                            .is_some_and(|v| *v != crate::render::Visibility::Unseen)
                    })
                }
                (Place::Stratum(_), Some(p)) if p.place == self.place => {
                    let vis = p.vis();
                    crate::foes::lines(st, |q| {
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
        if let Some(rr) = r.route {
            f.render_widget(
                Paragraph::new(hud::route_row(&route, home_trace.as_deref(), rr.width)),
                rr,
            );
        }
        // Footer: four rows, the same four in every state.
        let slots = self.slots(st);
        let footer = self.footer(st, city, jacked, &slots, tether.is_some());
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
                Kind::Info => Style::default().fg(crate::ui::chrome::TEXT),
            };
            // Marked names lit, as the LOG pane draws them.
            let lit = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
            let marked = e.marked();
            let shift = marked.chars().count() - e.text.chars().count();
            let y = map_area.bottom().saturating_sub(1);
            for (i, ch) in marked.chars().enumerate() {
                let x = map_area.x + 1 + i as u16;
                if x >= map_area.right() {
                    break;
                }
                let on = i >= shift
                    && e.marks
                        .iter()
                        .any(|(a, b)| (a + shift..b + shift).contains(&i));
                put(
                    f.buffer_mut(),
                    x,
                    y,
                    &ch.to_string(),
                    if on { lit } else { style },
                    map_area.right(),
                );
            }
        }

        // Look: the info window, kept off the cursor.
        if let Some(l) = self.looking.as_ref()
            && let Some(at) = tile_to_screen(map_area, origin, l.cursor)
        {
            let reachable: Vec<Pos> = interact::gather(st, &|pl, p| self.inside_person_at(pl, p))
                .iter()
                .map(|s| s.pos)
                .collect();
            let map = self.map(st, city);
            let card = crate::look::look_card(
                st,
                &map,
                self.place,
                l.cursor,
                &|pl, p| self.inside_person_at(pl, p),
                &reachable,
            );
            let h = crate::look::box_height(&card, crate::look::LOOK_W);
            let r = crate::look::place_box(map_area, at, h);
            crate::look::draw_box(&card, r, f.buffer_mut());
        }
        // A bubbles scene dims the frame, a keypad goes over the map, the bubble last.
        if bubbles {
            use crate::missions::{bubbles, scene};
            bubbles::chrome(f.buffer_mut(), &r, st, self.console.is_some());
            if let Some(pad) = scene::running(st).and_then(|s| s.pad()) {
                crate::ui::dim::wash_rect(f.buffer_mut(), map_area);
                pad.draw(st, f);
            }
            bubbles::say(f.buffer_mut(), map_area, origin, st);
        }
        if self.help {
            help::draw(f, self.mode, st.dev_unlocked);
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

    /// The lot with you at (6,8), the goon at `goon`, the map on foot.
    fn lot(goon: Pos) -> (AppState, MapScreen) {
        let mut st = crate::foes::testutil::lot(goon);
        let s = MapScreen::walking(&mut st);
        (st, s)
    }

    fn draw_buffer(scr: &dyn Screen, st: &AppState, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| scr.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    /// The screen cell a tile is drawn on: the map's origin is read back from
    /// where your own `@` sits.
    fn cell_of(
        s: &MapScreen,
        st: &AppState,
        buf: &ratatui::buffer::Buffer,
        p: Pos,
    ) -> (String, Color) {
        let c = cell_at(s, st, buf, p);
        (c.symbol().to_string(), c.fg)
    }

    /// The whole screen cell a tile is drawn on, found as `cell_of` finds it.
    fn cell_at<'b>(
        s: &MapScreen,
        st: &AppState,
        buf: &'b ratatui::buffer::Buffer,
        p: Pos,
    ) -> &'b ratatui::buffer::Cell {
        let me = st.player.as_ref().unwrap().pos;
        let area = s.last_map.get();
        let mut found = None;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if buf[(x, y)].symbol() == "@" && buf[(x, y)].fg == Color::Rgb(255, 255, 255) {
                    found = Some((x as i32, y as i32));
                }
            }
        }
        let (mx, my) = found.expect("your @ is on the map");
        let (sx, sy) = ((mx + p.x - me.x) as u16, (my + p.y - me.y) as u16);
        &buf[(sx, sy)]
    }

    #[test]
    fn a_standing_foe_blocks_a_step_and_a_down_one_does_not() {
        let (mut st, mut s) = lot(Pos::new(7, 8));
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(6, 8));
        assert!(logged(&st, "the goon is in the way"));
        st.foes[0].down = true;
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(7, 8));
    }

    #[test]
    fn the_goon_is_drawn_red_and_down_as_a_percent() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        s.update(&mut st, &[]);
        let buf = draw_buffer(&s, &st, 100, 30);
        assert_eq!(
            cell_of(&s, &st, &buf, Pos::new(14, 8)),
            ("@".to_string(), Color::Rgb(255, 80, 80))
        );
        st.foes[0].down = true;
        let buf = draw_buffer(&s, &st, 100, 30);
        assert_eq!(
            cell_of(&s, &st, &buf, Pos::new(14, 8)),
            ("%".to_string(), Color::Rgb(120, 30, 30))
        );
    }

    #[test]
    fn e_on_the_street_both_ways_and_still_hunted() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        // The first frame: the goon sees you and the street engages.
        s.update(&mut st, &[]);
        assert!(st.net.engaged.as_ref().is_some_and(|e| !e.manual));
        assert!(logged(&st, "ENGAGED · the goon has you"));
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("still hunted"));
        st.foes[0].down = true;
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("calm"));
        assert!(st.net.engaged.is_none());
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("nothing to engage"));
        // An idle goon in sight memory can be engaged by hand.
        st.foes.push(crate::foes::Foe::goon(
            Place::Stratum(StratumKind::Street),
            Pos::new(18, 8),
        ));
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_none(), "twelve tiles: unseen by the goon");
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.as_ref().is_some_and(|e| e.manual));
        assert_eq!(last(&st), Some("engaged"));
        // Leaving the map ends it.
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Skyways);
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_none());
    }

    #[test]
    fn console_engage_and_calm_work_on_the_street() {
        let (mut st, _s) = lot(Pos::new(18, 8));
        let at = game::At {
            place: Place::Stratum(StratumKind::Street),
            pos: Pos::new(6, 8),
        };
        assert_eq!(
            crate::screens::console::run(&mut st, at, "engage").unwrap(),
            "engaged by hand"
        );
        assert_eq!(
            crate::screens::console::run(&mut st, at, "calm").unwrap(),
            "calm"
        );
    }

    /// The lot, the goon seeing you, the fight on, you skilled; `space` stays and
    /// opens the act half.
    fn act_half(goon: Pos) -> (AppState, MapScreen) {
        let (mut st, mut s) = lot(goon);
        st.character.skills.insert(game::SkillId::new("handguns"));
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_some() && crate::net::your_turn(&st));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Act));
        (st, s)
    }

    #[test]
    fn the_act_half_opens_the_cursor_on_the_nearest_foe() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        assert_eq!(s.target, Some(Pos::new(14, 8)));
        let buf = draw_buffer(&s, &st, 100, 30);
        let text = buffer_text(&buf);
        assert!(text.contains("goon · Scrap nine · 8 tiles · 74%"), "{text}");
        let me = st.player.as_ref().unwrap().pos;
        let _ = me;
        assert_eq!(
            cell_of(&s, &st, &buf, Pos::new(14, 8)),
            ("@".to_string(), Color::Rgb(0, 0, 0))
        );
        // The line of fire between you and the goon.
        assert_eq!(cell_of(&s, &st, &buf, Pos::new(10, 8)).0, "·");
        // The turn ends: the cursor goes.
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(s.target, None);
    }

    #[test]
    fn the_line_of_fire_leaves_a_foe_in_between_drawn() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        st.foes.push(crate::foes::Foe::goon(
            Place::Stratum(StratumKind::Street),
            Pos::new(9, 8),
        ));
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(s.target, Some(Pos::new(14, 8)));
        let buf = draw_buffer(&s, &st, 100, 30);
        assert_eq!(
            cell_of(&s, &st, &buf, Pos::new(9, 8)),
            ("@".to_string(), Color::Rgb(255, 80, 80)),
            "the nearer goon stands over the tether"
        );
        assert_eq!(cell_of(&s, &st, &buf, Pos::new(10, 8)).0, "·");
    }

    #[test]
    fn tab_cycles_foes_nearest_first() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        st.foes.push(crate::foes::Foe::goon(
            Place::Stratum(StratumKind::Street),
            Pos::new(9, 8),
        ));
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(s.target, Some(Pos::new(9, 8)));
        s.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(s.target, Some(Pos::new(14, 8)));
        s.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(s.target, Some(Pos::new(9, 8)));
        // Tab is still the deck's: a stim can be used mid-fight.
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Tab)]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn your_turn_ends_on_its_own_when_the_ap_is_spent() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        st.force_hit = Some(true);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 0);
        assert!(crate::net::your_turn(&st), "a beat to read the numbers");
        st.tick += crate::net::AUTO_END_TICKS - 1;
        s.update(&mut st, &[]);
        assert!(crate::net::your_turn(&st));
        st.tick += 2;
        s.update(&mut st, &[]);
        assert!(!crate::net::your_turn(&st), "the turn passed on its own");
    }

    #[test]
    fn the_turn_column_sits_top_right_and_lights_the_live_half() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        s.update(&mut st, &[]);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains(" TURN 1 "), "{text}");
        assert!(text.contains(" ▶ YOU            "), "{text}");
        assert!(text.contains(" ◆ MOV ▰▰▰▰    4/4"), "{text}");
        assert!(text.contains("   AP  ●●      2/2"), "{text}");
        assert!(text.contains("   goon        30 "), "{text}");
        // The column's right edge is the map's right edge, its top the map's top row.
        let rows: Vec<&str> = text.lines().collect();
        let r = rows.iter().position(|l| l.contains(" TURN 1 ")).unwrap();
        assert_eq!(r, 5, "the map's first row");
        assert!(
            rows[r + 1].find(" ▶ YOU").is_some_and(|c| c + 18 == 59),
            "{:?}",
            rows[r + 1]
        );
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("   MOV ▰▰▰▰    4/4"), "{text}");
        assert!(text.contains(" ◆ AP  ●●      2/2"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("   YOU            "), "{text}");
        assert!(text.contains(" ▶ goon        30 "), "{text}");
        assert!(text.contains("   moves 3 › fires"), "{text}");
        // Calm: no column.
        st.foes[0].down = true;
        crate::net::disengage(&mut st);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(!text.contains("TURN 1"), "{text}");
    }

    #[test]
    fn the_turn_column_reads_the_ice_in_a_construct() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        engaged_at_the_gate(&mut st, &mut s);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains(" ▶ YOU            "), "{text}");
        assert!(text.contains("   grey ICE   6/6 "), "{text}");
    }

    #[test]
    fn the_cursor_comes_back_to_the_last_target() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        st.foes.push(crate::foes::Foe::goon(
            Place::Stratum(StratumKind::Street),
            Pos::new(9, 8),
        ));
        st.foes[1].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 8),
            lost: 0,
        };
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(s.target, Some(Pos::new(9, 8)), "the nearer hunter first");
        s.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(s.target, Some(Pos::new(14, 8)));
        st.force_hit = Some(false);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        settle(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let far = st.foes[0].pos;
        assert_eq!(s.target, Some(far), "the same goon as last round");
        // Down, it is not waited for: the cursor goes to the nearest hunter.
        st.foes[0].down = true;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        settle(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(s.target, Some(st.foes[1].pos));
    }

    #[test]
    fn the_act_half_in_a_construct_aims_at_the_ice_and_space_attacks() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = engaged_at_the_gate(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let at = game::At {
            place: Place::Construct(b),
            pos: st.player.as_ref().unwrap().pos,
        };
        // The pawnshop deck's two slots: copy out, attack in.
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        crate::screens::console::run(&mut st, at, "program attack").unwrap();
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Move));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Act));
        let guard = st.constructs.get(b).unwrap().guards[0].pos;
        assert_eq!(s.target, Some(guard), "the cursor opens on the awake piece");
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("grey ICE · 6/6 ·"), "{text}");
        assert!(text.contains("space attack"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
        assert_eq!(last(&st), Some("attack · grey ICE 4/6"), "{:?}", last(&st));
        let shapes: Vec<&game::Fx> = st.fx.iter().map(|i| &i.fx).collect();
        assert!(
            shapes.iter().any(|f| matches!(f, game::Fx::Streak { .. }))
                && shapes.iter().any(|f| matches!(f, game::Fx::Tear))
                && st.fx.iter().any(|i| i.label == "-2"),
            "{shapes:?}"
        );
        assert!(
            !shapes.iter().any(|f| matches!(f, game::Fx::Bolt { .. })),
            "the attack has the shot's look, not the trace's"
        );
    }

    #[test]
    fn look_at_a_goon_reads_it_and_a_body_reads_down() {
        let (mut st, mut s) = lot(Pos::new(8, 8));
        s.update(&mut st, &[]);
        // `x` opens the look, two steps east put the cursor on the goon.
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        s.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('l'))]);
        assert_eq!(s.looking.as_ref().map(|l| l.cursor), Some(Pos::new(8, 8)));
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("Goon"), "{text}");
        assert!(text.contains("hunting · 30/30 · Scrap nine"), "{text}");
        st.foes[0].down = true;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("Goon"), "{text}");
        assert!(text.contains("down"), "{text}");
    }

    #[test]
    fn s_is_refused_on_the_lot() {
        let (mut st, mut s) = lot(Pos::new(18, 8));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Stay
        ));
        assert_eq!(last(&st), Some("nothing to save on the lot"));
    }

    #[test]
    fn a_blocked_step_costs_no_move() {
        let (mut st, mut s) = lot(Pos::new(9, 8));
        st.player.as_mut().unwrap().pos = Pos::new(1, 8);
        s.update(&mut st, &[]);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Move));
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(1, 8));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 4);
    }

    #[test]
    fn space_fires_and_spends_an_ap() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        st.force_hit = Some(true);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
        assert!(logged(&st, "you fire · goon 4 · 4 · 4"), "{:?}", last(&st));
        assert_eq!(st.fx.len(), 15, "five shapes a round");
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("AP ●○ 1/2"), "{text}");
    }

    #[test]
    fn space_off_a_foe_says_pick_a_target() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(s.target, Some(Pos::new(15, 8)));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("pick a target"));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 2);
        assert!(st.fx.is_empty());
    }

    #[test]
    fn esc_drops_the_cursor() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert_eq!(s.target, None);
        // Any direction brings it back, from your own tile.
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(s.target, Some(Pos::new(7, 8)));
        s.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(s.target, Some(Pos::new(14, 8)));
    }

    #[test]
    fn a_direct_step_spends_a_move_and_the_last_opens_the_act_half() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        s.update(&mut st, &[]);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Move));
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(5, 8));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 3);
        for _ in 0..3 {
            s.update(&mut st, &[key(KeyCode::Char('h'))]);
        }
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(2, 8));
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Act));
        assert_eq!(s.target, Some(Pos::new(14, 8)));
        // Moves are done: a direction now moves the cursor, not you.
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(2, 8));
        assert_eq!(s.target, Some(Pos::new(13, 8)));
    }

    #[test]
    fn g_aims_a_path_on_the_street_and_enter_walks_it() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char('g'))]);
        assert_eq!(s.aim, Some(Pos::new(6, 8)));
        s.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('l'))]);
        assert_eq!(s.aim, Some(Pos::new(8, 8)));
        let (path, lit) = s.aim_path(&st).unwrap();
        assert_eq!(path, vec![Pos::new(7, 8), Pos::new(8, 8)]);
        assert_eq!(lit, 4);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(s.walking.is_some());
        for _ in 0..3 {
            st.tick += crate::net::WALK_TICKS;
            s.update(&mut st, &[]);
        }
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(8, 8));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 2);
        assert!(s.walking.is_none());
    }

    #[test]
    fn street_keys_on_the_foes_turn() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(!crate::net::your_turn(&st));
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("not your turn"));
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("not your turn"));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("not your turn"));
        assert!(st.net.engaged.is_some());
    }

    #[test]
    fn number_keys_in_a_street_fight() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        st.character.give(game::ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, game::ItemId::new("neural_port"));
        let list = MapScreen::act_list(&st);
        let slot = |id: &str| {
            let i = list.iter().position(|a| *a == act_ability(id)).unwrap();
            char::from_digit(i as u32 + 1, 10).unwrap()
        };
        st.net.engaged.as_mut().unwrap().ap = 1;
        // Static wants the cursor on a foe.
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        s.update(&mut st, &[key(KeyCode::Char(slot("static")))]);
        assert_eq!(last(&st), Some("pick a target"));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
        s.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(s.target, Some(Pos::new(14, 8)));
        s.update(&mut st, &[key(KeyCode::Char(slot("static")))]);
        assert_eq!(last(&st), Some("no AP left"));
        // With both points it fires.
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        st.character.give(game::ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, game::ItemId::new("neural_port"));
        st.force_hit = Some(true);
        s.update(&mut st, &[key(KeyCode::Char(slot("static")))]);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 0);
        assert_eq!(st.foes[0].stunned, 1);
        assert!(logged(&st, "goon stunned"));
    }

    #[test]
    fn the_footer_reads_engaged_and_the_foe_line() {
        let (mut st, mut s) = act_half(Pos::new(14, 8));
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("ENGAGED"), "{text}");
        assert!(!text.contains("HUNTED"), "{text}");
        assert!(
            text.contains("▣ goon · hunting · 30/30 · 8 tiles"),
            "{text}"
        );
        assert!(text.contains("TURN 1  YOU › goon"), "{text}");
        assert!(
            text.contains("hjkl target · t next · space fire · 1-9 act · . end turn"),
            "{text}"
        );
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("THE GOON ACTS"), "{text}");
    }

    /// The whole street fight from the title's COMBAT entry: walk into the goon's
    /// sight, aim, fire until it drops, calm.
    #[test]
    fn a_whole_street_fight() {
        let mut st = AppState::new();
        st.unlock_dev();
        let street = crate::screens::title::CombatMap::Street;
        let ScreenCmd::Switch { to: map, .. } =
            crate::screens::title::combat_start(&mut st, street)
        else {
            panic!("the lot")
        };
        let _ = map;
        let mut s = MapScreen::walking(&mut st);
        st.force_hit = Some(true);
        // Four tiles east: at eight tiles the goon sees you.
        // East along the road: at (30,8) the goon in the far corner sees you.
        for _ in 0..40 {
            if st.net.engaged.is_some() {
                break;
            }
            s.update(&mut st, &[key(KeyCode::Char('l'))]);
        }
        assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(30, 8));
        assert!(st.net.engaged.is_some(), "{:?}", last(&st));
        assert!(logged(&st, "ENGAGED · the goon has you"));
        let mut rounds = 0;
        while !st.foes[0].down && rounds < 6 {
            settle(&mut st, &mut s);
            assert!(crate::net::your_turn(&st));
            if crate::net::phase(&st) == Some(crate::net::Phase::Move) {
                s.update(&mut st, &[key(KeyCode::Char(' '))]);
            }
            assert_eq!(s.target, Some(st.foes[0].pos), "the cursor on the goon");
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(logged(&st, "you fire · goon 4 · 4 · 4"), "{:?}", last(&st));
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
            rounds += 1;
        }
        assert!(st.foes[0].down, "{rounds} turns: {:?}", st.foes[0]);
        assert!(logged(&st, "goon down"));
        // The round ends with nothing hunting: calm.
        settle(&mut st, &mut s);
        assert!(st.net.engaged.is_none(), "{:?}", st.net.engaged);
        assert!(logged(&st, "calm"));
        assert!(st.character.hp > 0);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(!text.contains("ENGAGED"), "{text}");
    }

    #[test]
    fn console_foe_and_foes() {
        let (mut st, _s) = lot(Pos::new(14, 8));
        let at = game::At {
            place: Place::Stratum(StratumKind::Street),
            pos: Pos::new(6, 8),
        };
        assert_eq!(
            crate::screens::console::run(&mut st, at, "foe").unwrap(),
            "a goon at 12,8"
        );
        assert_eq!(st.foes.len(), 2);
        assert_eq!(st.foes[1].pos, Pos::new(12, 8));
        assert_eq!(
            crate::screens::console::run(&mut st, at, "foe 40").unwrap_err(),
            "no room there"
        );
        let listed = crate::screens::console::run(&mut st, at, "foes").unwrap();
        assert!(listed.contains("0 goon idle 30/30 at 14,8"), "{listed}");
        assert!(listed.contains("1 goon idle 30/30 at 12,8"), "{listed}");
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
    use ratatui::buffer::Buffer;

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
    fn the_stairs_see_the_new_stratum_by_the_minute() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap();
        st.world = Some(sim::World::seed(
            city,
            &st.people,
            &sim::SimParams::default(),
            hud::START_MINUTE_OF_DAY,
            (12 * 60 + 24 * 60 - 22 * 60) as u64 * sim::TICKS_PER_MINUTE,
        ));
        assert_eq!(st.clock(), (12, 0));
        let stair = city
            .connectors
            .iter()
            .filter(|c| c.kind != citygen::ConnectorKind::Jack)
            .find(|c| {
                c.serves_from(StratumKind::Street)
                    .is_some_and(|(_, above)| above == Some(StratumKind::Skyways))
                    && city.walkable(StratumKind::Skyways, c.pos)
            })
            .expect("a public way up")
            .pos;
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), stair);
        s.update(&mut st, &[key(KeyCode::Char('>'))]);
        let (place, _) = player(&st);
        assert_eq!(place, Place::Stratum(StratumKind::Skyways));
        let on_arrival = st
            .player
            .as_ref()
            .unwrap()
            .vis()
            .count(|v| *v == Visibility::Visible);
        let bonus = st.sight_bonus();
        let minute = st.minute_of_day();
        let city = st.city.as_ref().unwrap();
        let noon = MapRef::stratum_at(city, StratumKind::Skyways, minute);
        st.player.as_mut().unwrap().update_fov(&noon, bonus);
        let fresh = st
            .player
            .as_ref()
            .unwrap()
            .vis()
            .count(|v| *v == Visibility::Visible);
        assert_eq!(on_arrival, fresh, "the stairs already saw noon");
    }

    #[test]
    fn explore_and_walk_round_trip() {
        let mut st = state();
        st.unlock_dev();
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
        // Saving saves the run where it stands, no screen over the map; the map keeps
        // its state.
        s.place = Place::Stratum(StratumKind::Undercity);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Stay
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
        st.unlock_dev();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        let before = s.cursor;
        s.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Down)]);
        assert_eq!(s.cursor, Pos::new(before.x + 1, before.y + 1));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('g'))]),
            ScreenCmd::Switch { .. }
        ));
        // r builds a fresh city and walks into it at once.
        let seed = st.city.as_ref().map(|c| c.seed);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('r'))]),
            ScreenCmd::Switch { .. }
        ));
        assert_ne!(st.city.as_ref().map(|c| c.seed), seed);
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
        st.unlock_dev();
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
    fn space_jacks_in_and_the_net_is_the_fifth_stratum() {
        let mut st = state();
        st.unlock_dev();
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
            jack_in(&mut st, &mut s),
            ScreenCmd::Pop {
                fx: TransitionKind::Glitch
            }
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
            jack_out(&mut st, &mut s),
            ScreenCmd::Pop {
                fx: TransitionKind::Glitch
            }
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
        st.unlock_dev();
        st.set_dev(false);
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
        st.unlock_dev();
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
        st.unlock_dev();
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
        st.unlock_dev();
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
        assert!(buffer_text(t.backend().buffer()).contains("save game"));
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
    }

    #[test]
    fn deck_opened_from_the_free_camera_links_to_the_cursor() {
        let mut st = state();
        st.unlock_dev();
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
            text.contains("x  Look")
                && text.contains("g  Take all")
                && text.contains("a  Take: Copper coil ×2")
                && text.contains("b  Take: Dead phone"),
            "{text}"
        );
        assert!(!text.contains("HERE"), "{text}");
        p.update(&mut st, &[key(KeyCode::Char('g'))]);
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
        assert!(text.contains("x  Look") && !text.contains("Take"), "{text}");
        // Look asks the map for its cursor on the tile and closes.
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('x'))]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(st.look_at, Some(pos));
    }

    /// A row as the window prints it, without the spacing: "i Interact: Terminal".
    fn row_text(c: &Choice) -> String {
        let thing = c
            .thing
            .as_ref()
            .map(|(t, _)| format!(": {t}"))
            .unwrap_or_default();
        format!("{} {}{thing}{}", c.key, c.label, c.tail)
    }

    /// A street tile with nothing in reach: no jack, no connector, no one, nothing
    /// on the ground.
    fn bare_sidewalk(st: &AppState) -> Pos {
        let city = st.city.as_ref().unwrap();
        let k = StratumKind::Street;
        let size = city.size;
        (0..size.h as i32)
            .flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| {
                city.walkable(k, *p)
                    && std::iter::once(*p)
                        .chain(p.neighbors8())
                        .all(|q| city.connectors_at(q).next().is_none())
            })
            .expect("a bare sidewalk")
    }

    #[test]
    fn nothing_in_reach_gathers_no_subjects() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        assert!(crate::interact::gather(&st, &|_, _| None).is_empty());
    }

    #[test]
    fn a_terminal_beside_you_is_one_subject_with_interact_and_jack() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = {
            let city = st.city.as_ref().unwrap();
            let jack = city
                .connectors
                .iter()
                .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
                .unwrap()
                .pos;
            jack.neighbors8()
                .find(|n| city.walkable(StratumKind::Street, *n))
                .unwrap()
        };
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        let subs = crate::interact::gather(&st, &|_, _| None);
        let t = subs
            .iter()
            .find(|s| {
                s.header
                    .as_ref()
                    .is_some_and(|h| h.name == "Access terminal")
            })
            .expect("the terminal is a subject");
        let rows: Vec<String> = t.choices.iter().map(row_text).collect();
        assert_eq!(rows, ["i Interact: Terminal", "j Jack in"]);
        assert!(t.dir.is_some());
    }

    #[test]
    fn take_rows_stop_when_the_letters_run_out() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (k, pos) = player(&st);
        let ids: Vec<game::ItemId> = st
            .content
            .items
            .iter()
            .map(|i| i.id.clone())
            .take(crate::interact::FREE_LETTERS.len() + 3)
            .collect();
        assert!(ids.len() > crate::interact::FREE_LETTERS.len());
        for id in ids {
            st.ground.add(k, pos, id, 1);
        }
        let here = crate::interact::gather(&st, &|_, _| None)
            .into_iter()
            .find(|s| s.dir.is_none())
            .expect("the stacks make a here subject");
        let keys: Vec<char> = here.choices.iter().map(|c| c.key).collect();
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            keys.len(),
            "every row has its own key: {keys:?}"
        );
        assert_eq!(keys[0], 'g');
    }

    #[test]
    fn the_window_puts_look_first_on_x() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        let (k, pos) = player(&st);
        st.ground.add(k, pos, game::ItemId::new("copper_coil"), 2);
        let here = crate::interact::gather(&st, &|_, _| None).remove(0);
        assert_eq!(here.dir, None);
        assert_eq!(
            here.header.as_ref().map(|h| h.name.as_str()),
            Some("Copper coil ×2")
        );
        let w = here.window(0);
        assert_eq!(w.choices()[0].key, 'x');
        assert!(matches!(w.choices()[0].pick, Pick::Look(p) if p == pos));
        assert_eq!(row_text(&w.choices()[1]), "g Take: Copper coil ×2");
    }

    /// Beside a public terminal with two stacks underfoot: two subjects, here and the
    /// terminal. Returns the player's tile and the terminal's.
    fn by_the_terminal_with_stacks(st: &mut AppState, s: &mut MapScreen) -> (Pos, Pos) {
        let (jack, at) = {
            let city = st.city.as_ref().unwrap();
            city.connectors
                .iter()
                .filter(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
                .find_map(|k| {
                    k.pos
                        .neighbors4()
                        .find(|n| {
                            city.walkable(StratumKind::Street, *n)
                                && city.connectors_at(*n).next().is_none()
                        })
                        .map(|n| (k.pos, n))
                })
                .unwrap()
        };
        stand(st, s, Place::Stratum(StratumKind::Street), at);
        let k = Place::Stratum(StratumKind::Street);
        st.ground.add(k, at, game::ItemId::new("copper_coil"), 2);
        st.ground.add(k, at, game::ItemId::new("dead_phone"), 1);
        (at, jack)
    }

    /// The key that steps in `d`.
    fn dir_key(d: Dir) -> KeyCode {
        KeyCode::Char(match d {
            Dir::N => 'k',
            Dir::S => 'j',
            Dir::E => 'l',
            Dir::W => 'h',
            Dir::NE => 'u',
            Dir::NW => 'y',
            Dir::SE => 'n',
            Dir::SW => 'b',
        })
    }

    /// Jacks in as the player does: Space, the terminal's window (through the
    /// which-way pick when several things are in reach), `j`. Returns what the window
    /// answered; the map has followed.
    fn jack_in(st: &mut AppState, s: &mut MapScreen) -> ScreenCmd {
        let mut cmd = s.update(st, &[key(KeyCode::Char(' '))]);
        if let Some(w) = s.which_way.as_ref() {
            let d = w
                .subjects
                .iter()
                .find(|x| x.choices.iter().any(|c| c.key == 'j'))
                .expect("a terminal in reach")
                .dir;
            cmd = s.update(st, &[key(d.map_or(KeyCode::Char(' '), dir_key))]);
        }
        let ScreenCmd::Push { to: mut w, .. } = cmd else {
            panic!("space opens the terminal's window")
        };
        let out = w.update(st, &[key(KeyCode::Char('j'))]);
        s.update(st, &[]);
        out
    }

    /// Jacks out as the player does: `J`, then `y`. Returns what the confirm answered.
    fn jack_out(st: &mut AppState, s: &mut MapScreen) -> ScreenCmd {
        let ScreenCmd::Push { to: mut ask, .. } = s.update(st, &[key(KeyCode::Char('J'))]) else {
            panic!("J asks: {:?}", last(st))
        };
        let out = ask.update(st, &[key(KeyCode::Char('y'))]);
        s.update(st, &[]);
        out
    }

    #[test]
    fn several_subjects_ask_which_way_and_a_direction_opens_one() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (at, jack) = by_the_terminal_with_stacks(&mut st, &mut s);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
        assert!(s.which_way.is_some(), "two subjects: the pick is up");
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains("WHICH WAY?")
                && text.contains("Access terminal")
                && text.contains("· here: "),
            "{text}"
        );
        let ScreenCmd::Push { to: w, .. } = s.update(&mut st, &[key(dir_between(at, jack))]) else {
            panic!("the terminal's direction opens its window")
        };
        assert!(s.which_way.is_none());
        st.tick += 40;
        let text = draw_screen(&*w, &st, 120, 40);
        assert!(
            text.contains("Access terminal") && text.contains("i  Interact: Terminal"),
            "{text}"
        );
        // Space opens your own tile.
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let ScreenCmd::Push { to: w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!("space opens here")
        };
        st.tick += 40;
        assert!(draw_screen(&*w, &st, 120, 40).contains("g  Take all"));
    }

    #[test]
    fn which_way_opens_the_direction_and_ignores_empty_ones() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (at, jack) = by_the_terminal_with_stacks(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let empty = Dir::ALL
            .into_iter()
            .map(|d| at.step(d))
            .find(|p| *p != jack && p.chebyshev(jack) > 0)
            .unwrap();
        assert!(matches!(
            s.update(&mut st, &[key(dir_between(at, empty))]),
            ScreenCmd::Stay
        ));
        assert!(
            s.which_way.is_some(),
            "a direction with nothing keeps the pick"
        );
        assert_eq!(player(&st).1, at, "and never walks");
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert!(s.which_way.is_none(), "esc puts it away, no pause menu");
    }

    #[test]
    fn one_subject_opens_at_once_and_none_opens_look_only() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        let ScreenCmd::Push { to: w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!("space opens a window")
        };
        assert!(s.which_way.is_none());
        st.tick += 40;
        let text = draw_screen(&*w, &st, 120, 40);
        assert!(text.contains("x  Look"), "{text}");
        assert!(
            text.lines()
                .filter(|l| l.contains("  ") && l.chars().any(|c| c.is_alphabetic()))
                .count()
                == 1,
            "look is the only row, no header: {text}"
        );
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
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  Jack in"));
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
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  Jack out"));
        let ScreenCmd::Replace { to: mut ask, .. } = p.update(&mut st, &[key(KeyCode::Char('j'))])
        else {
            panic!("jack out asks first")
        };
        st.tick += 40;
        assert!(draw_screen(&*ask, &st, 120, 40).contains("Jack out?"));
        ask.update(&mut st, &[key(KeyCode::Char('y'))]);
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
        assert!(draw_screen(&*p, &st, 120, 40).contains("<  Go down: Undercity"));
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
            text.contains("Abilities") && text.contains("a  Netscan · ready"),
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
    fn a_level_plays_its_upload_when_the_map_is_back() {
        let mut st = crate::jobs::testutil::state_home();
        let mut s = MapScreen::walking(&mut st);
        let uploads = |st: &AppState| {
            st.fx
                .iter()
                .filter(|f| matches!(f.fx, game::Fx::Upload))
                .cloned()
                .collect::<Vec<_>>()
        };
        // two levels at once, gained off the map: one upload waits, the last level's
        assert_eq!(st.gain_xp(200 + 400), 2);
        assert_eq!(st.levelled, Some(3));
        assert!(uploads(&st).is_empty(), "it waits for the map");
        // the map on top but not walking (the free camera): it waits still
        s.mode = Mode::Explore;
        st.tick += 1;
        s.update(&mut st, &[]);
        assert!(
            uploads(&st).is_empty() && st.levelled == Some(3),
            "not walking"
        );
        s.mode = Mode::Walk;
        st.tick += 1;
        s.update(&mut st, &[]);
        let played = uploads(&st);
        assert_eq!(played.len(), 1);
        let me = st.player.as_ref().unwrap();
        assert_eq!((played[0].place, played[0].origin), (me.place, me.pos));
        assert_eq!(played[0].label, "LEVEL 3");
        assert_eq!(played[0].started, st.tick);
        assert_eq!(st.levelled, None);
        // short of a level, nothing waits
        st.gain_xp(10);
        assert_eq!(st.levelled, None);
    }

    /// Seed 8 moved in with the news out (the welcome put by), you standing `(dx, dy)`
    /// from the holo set and looking around, one frame run.
    fn by_the_holo_set(dx: i32, dy: i32) -> (AppState, MapScreen) {
        let mut st = crate::jobs::testutil::state_home();
        let mut s = MapScreen::walking(&mut st);
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::news::put_out(&mut st, "marlo", None);
        assert!(crate::missions::news::current(&st).is_some_and(|n| n.on));
        let (place, pos) = crate::missions::news::set_tile(&st).unwrap();
        let mut me = crate::walker::Player::at(place, pos.offset(dx, dy));
        me.update_fov(
            &MapRef::for_place(
                st.city.as_ref().unwrap(),
                &st.interiors,
                &st.constructs,
                place,
                0,
            )
            .unwrap(),
            0,
        );
        st.player = Some(me);
        s.update(&mut st, &[]);
        (st, s)
    }

    /// The cells that change when the set goes off, drawn at 120x40 on frame tick 24:
    /// on, then off.
    fn holo_on_off(st: &mut AppState, s: &MapScreen) -> (Buffer, Buffer, Vec<(u16, u16)>) {
        st.tick = 24;
        let on = draw_buffer(s, st, 120, 40);
        st.missions.news.as_mut().unwrap().on = false;
        let off = draw_buffer(s, st, 120, 40);
        st.missions.news.as_mut().unwrap().on = true;
        let changed = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|p| on[*p] != off[*p])
            .collect();
        (on, off, changed)
    }

    #[test]
    fn the_holo_set_plays_in_magenta_while_it_is_on() {
        // In the living room, the set in view.
        let (mut st, s) = by_the_holo_set(0, 1);
        let (on, off, changed) = holo_on_off(&mut st, &s);
        assert_eq!(changed.len(), 1, "{changed:?}");
        let (on, off) = (&on[changed[0]], &off[changed[0]]);
        assert_eq!(off.symbol(), "◫");
        assert_eq!(on.fg, crate::ui::chrome::MAGENTA);
        assert!(
            st.style
                .glyphs
                .holo
                .iter()
                .any(|c| on.symbol() == c.to_string())
        );
    }

    #[test]
    fn a_holo_ad_cutting_in_glows_magenta() {
        let mut st = crate::jobs::testutil::state_home();
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::news::put_out(&mut st, "marlo", None);
        let ad = crate::missions::news::tests::an_ad_cutting_in(&mut st);
        let s = MapScreen::walking(&mut st);
        assert!(s.place.is_street());
        // The cut's cell alone, the bubble over it taken away.
        st.missions.asides.clear();
        st.tick = 24;
        let on = draw_buffer(&s, &st, 120, 40);
        let cut = st.missions.news.as_mut().unwrap().cut.take();
        let off = draw_buffer(&s, &st, 120, 40);
        st.missions.news.as_mut().unwrap().cut = cut;
        let changed: Vec<(u16, u16)> = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|p| on[*p] != off[*p])
            .collect();
        assert_eq!(changed.len(), 1, "{changed:?}");
        let (on, off) = (&on[changed[0]], &off[changed[0]]);
        assert_eq!(on.fg, crate::ui::chrome::MAGENTA);
        assert_eq!(on.symbol(), off.symbol(), "the glyph kept");
        assert!(
            st.style
                .glyphs
                .holo
                .iter()
                .any(|c| on.symbol() == c.to_string())
        );
        assert_eq!(crate::missions::news::cut_in(&st), Some(ad));
    }

    #[test]
    fn a_story_line_torn_through_an_ad_glows_magenta() {
        use crate::missions::{Mark, ad, marks, news};
        let mut st = crate::jobs::testutil::state_home();
        ad::tests::ad_then_after(&mut st);
        // Put up in the flat; out on the street by an ad, the next update tears it through.
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        crate::missions::begin(&mut st);
        let ad = news::tests::by_an_ad(&mut st);
        let mut s = MapScreen::walking(&mut st);
        assert!(s.place.is_street());
        crate::app::Screen::update(&mut s, &mut st, &[]);
        assert_eq!(ad::cut_in(&st), Some(ad));
        assert_eq!(news::cut_in(&st), None, "no news out");
        // The cut's cell alone, the bubble over it taken away.
        st.missions.asides.clear();
        st.tick = 24;
        let on = draw_buffer(&s, &st, 120, 40);
        let cut = st.missions.ad_cut.take();
        let off = draw_buffer(&s, &st, 120, 40);
        st.missions.ad_cut = cut;
        let changed: Vec<(u16, u16)> = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|p| on[*p] != off[*p])
            .collect();
        assert_eq!(changed.len(), 1, "{changed:?}");
        let (on, off) = (&on[changed[0]], &off[changed[0]]);
        assert_eq!(on.fg, crate::ui::chrome::MAGENTA);
        assert_eq!(on.symbol(), off.symbol(), "the glyph kept");
    }

    #[test]
    fn an_unseen_holo_set_does_not_play_through_the_wall() {
        // In the bedroom, its wall between you and the set.
        let (mut st, s) = by_the_holo_set(-10, 1);
        let (_, pos) = crate::missions::news::set_tile(&st).unwrap();
        let seen = st.player.as_ref().unwrap().vis().get(pos).copied();
        assert_ne!(seen, Some(Visibility::Visible));
        let (_, _, changed) = holo_on_off(&mut st, &s);
        assert_eq!(changed, [], "on or off, it looks the same");
    }

    #[test]
    fn asides_float_over_the_map_only_while_you_walk() {
        let (mut st, mut s) = by_the_holo_set(0, 1);
        assert_eq!(st.missions.asides.len(), 1, "the set reads its first line");
        // What the asides add to the frame.
        let added = |s: &MapScreen, st: &mut AppState| {
            let with = draw_buffer(s, st, 120, 40);
            let up = std::mem::take(&mut st.missions.asides);
            let without = draw_buffer(s, st, 120, 40);
            st.missions.asides = up;
            (0..40)
                .flat_map(|y| (0..120).map(move |x| (x, y)))
                .filter(|p| with[*p] != without[*p])
                .count()
        };
        assert!(added(&s, &mut st) > 20, "a bubble over the map");
        s.console = Some(Console::default());
        assert_eq!(added(&s, &mut st), 0, "none with the console open");
        s.console = None;
        s.mode = Mode::Explore;
        assert_eq!(added(&s, &mut st), 0, "none from the free camera");
        s.mode = Mode::Walk;
        // A scene starting takes them down.
        crate::missions::scene::play(&mut st, "waking");
        s.update(&mut st, &[]);
        assert!(crate::missions::scene::running(&st).is_some());
        assert!(st.missions.asides.is_empty());
    }

    #[test]
    fn a_rest_leaves_the_console_its_keys() {
        let mut st = crate::jobs::testutil::state_home();
        st.unlock_dev();
        let mut s = MapScreen::walking(&mut st);
        let until = crate::rest::now(&st) + 600;
        crate::rest::start(&mut st, crate::rest::Kind::Sleep, until);
        s.update(&mut st, &[key(KeyCode::Char(':'))]);
        assert!(s.console.is_some());
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert_eq!(s.console.as_ref().map(|c| c.input.as_str()), Some("x"));
        assert!(st.rest.is_some());
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
            assert_eq!(text.contains("Go down"), leads_down, "{:?}\n{text}", k.pos);
            assert_eq!(text.contains("Go up"), leads_up, "{:?}\n{text}", k.pos);
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
        st.unlock_dev();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char('v'))])
        else {
            panic!("v opens the view popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 100, 30);
        assert!(
            text.contains("View") && text.contains("a  ▣ Here") && text.contains("e  ▢ Inspect"),
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

    /// A game started without `--dev`: on foot, no badge, and the dev keys do nothing.
    #[test]
    fn without_dev_mode_unlocked_the_dev_keys_do_nothing() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        assert_eq!(s.mode, Mode::Walk, "no free camera");
        s.update(
            &mut st,
            &[
                key(KeyCode::Char('`')),
                key(KeyCode::Char('c')),
                key(KeyCode::F(5)),
                key(KeyCode::F(6)),
                key(KeyCode::Char(':')),
            ],
        );
        assert!(!st.dev, "the backtick is locked");
        assert_eq!(s.mode, Mode::Walk);
        assert!(!st.sidebar.is_on(Section::Inspect) && !st.sidebar.is_on(Section::Legend));
        assert!(s.console.is_none(), "no console");
        st.tick += 500;
        let text = draw_screen(&s, &st, 120, 40);
        let header = text.lines().next().unwrap();
        assert!(
            header.contains("VEX :: LVL 1 :: ¤ 320")
                && !header.contains("DEV")
                && !header.contains("fps")
                && !header.contains("seed"),
            "{header}"
        );
        let ScreenCmd::Push { to: popup, .. } = s.update(&mut st, &[key(KeyCode::Char('v'))])
        else {
            panic!("v opens the view popup")
        };
        st.tick += 40;
        let text = draw_screen(&*popup, &st, 100, 30);
        assert!(
            text.contains("Log") && !text.contains("Inspect") && !text.contains("Legend"),
            "{text}"
        );
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains("pause menu") && !text.contains("dev console"),
            "{text}"
        );
    }

    #[test]
    fn dev_off_clears_dev_sections() {
        let mut st = state();
        st.unlock_dev();
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
        st.unlock_dev();
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
                lines[2].starts_with(" FOCUS")
                    && !lines[2].contains("[F5]")
                    && lines[2].contains("· unseen"),
                "the band row the dev tabs left carries the exposure: {text}"
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
                    && lines[3].contains("[F1]Here* [F2]Jobs* [F3]Map* [F4]Log*"),
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
    fn the_toast_lights_the_names_it_marks() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        st.sidebar.set(Section::Log, false);
        st.say("Entering `Novbio Labs`");
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let text = buffer_text(buf);
        let (y, line) = text
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("Entering Novbio Labs"))
            .unwrap_or_else(|| panic!("{text}"));
        let x = line.chars().position(|c| c == 'N').unwrap() as u16;
        let e = line.chars().position(|c| c == 'E').unwrap() as u16;
        assert_eq!(buf[(x, y as u16)].fg, CYAN, "the name is lit");
        assert_ne!(buf[(e, y as u16)].fg, CYAN, "the rest is not");
    }

    #[test]
    fn f_keys_toggle_sections() {
        let mut st = state();
        st.unlock_dev();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::F(1))]);
        assert!(!st.sidebar.is_on(Section::Here));
        assert!(draw_screen(&s, &st, 100, 30).contains("[F1]Here [F2]Jobs*"));
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
        p.jack_in(&city, None, 0).unwrap();
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
        crate::jobs::testutil::sample_jobs(&mut st);
        st.say("took it");
        st.tick += 200;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("THE DROP"), "{text}");
        // The minimap's bottom row names the district you stand in.
        let city = st.city.as_ref().unwrap();
        let at = st.player.as_ref().unwrap().pos;
        let d = city
            .tile(StratumKind::Street, at)
            .and_then(|t| t.district)
            .map(|d| city.district(d).name.clone())
            .expect("a district under you");
        let side = |l: &str| l.chars().rev().take(40).collect::<Vec<_>>();
        let named = text.lines().any(|l| {
            let s: String = side(l).into_iter().rev().collect();
            s.trim_end() == format!(" {d}")
        });
        assert!(named, "minimap bottom row: {text}");
        assert!(text.contains("took it"), "log pane: {text}");
    }

    #[test]
    fn frame_carries_no_explicit_background() {
        let mut st = state();
        crate::jobs::testutil::sample_jobs(&mut st);
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
        st.unlock_dev();
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
        // someone standing on the street a few steps from you, as a walker draws: by night
        // most of this city's people are off the map (they have no home in it), so one
        // who's indoors steps out there
        let (id, at) = {
            let city = st.city.as_ref().unwrap();
            let you = st.player.as_ref().unwrap().pos;
            let world = st.world.as_ref().unwrap();
            let free = |q: Pos| {
                q != you && city.walkable(StratumKind::Street, q) && world.person_at(q).is_none()
            };
            let at = citygen::Rect::new(you.x - 6, you.y - 6, 13, 13)
                .iter()
                .filter(|q| free(*q) && q.neighbors4().any(free))
                .min_by_key(|q| q.manhattan(you))
                .expect("street near you");
            let who = world
                .people()
                .iter()
                .find(|p| p.tint.is_none() && matches!(p.whereabouts, sim::Whereabouts::Inside(_)))
                .expect("someone indoors");
            (who.id, at)
        };
        st.world.as_mut().unwrap().park(id, at);
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        let walker = sim::Mover {
            pos: at,
            kind: sim::MoverKind::Walker { id, tint: None },
        };
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
        assert!(
            text.contains(&card.name) && text.contains("t  Talk") && !text.contains("HERE"),
            "{text}"
        );
        assert!(matches!(
            here.update(&mut st, &[key(KeyCode::Char('t'))]),
            ScreenCmd::Stay
        ));
        assert!(
            last(&st).is_some_and(|l| l.starts_with(&format!("{}: \u{201c}", card.name))),
            "what they say is logged: {:?}",
            last(&st)
        );
        assert!(
            matches!(
                here.update(&mut st, &[key(KeyCode::Esc)]),
                ScreenCmd::Pop { .. }
            ),
            "the talk card closes to the game"
        );
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
            !text.contains("Talk"),
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

    /// A Medium city with a megablock and a home, put in place as a new game would.
    fn state_home() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 8,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    #[test]
    fn a_new_game_starts_with_your_own_card_and_code() {
        let mut st = state_home();
        let _ = MapScreen::walking(&mut st);
        let h = st.home.unwrap().home;
        let code = st
            .journal
            .code_for(h.building, h.level, h.number)
            .expect("your own code");
        assert_eq!(code.source, crate::credentials::Source::Home);
        assert_eq!(
            code.code,
            citygen::rng::door_code(
                st.city.as_ref().unwrap().seed,
                h.building,
                h.level,
                h.number
            )
        );
        let home_card = game::ItemId::new(crate::credentials::HOME_KEYCARD);
        assert_eq!(st.character.count(&home_card), 1);
        assert!(
            st.content
                .item(&home_card)
                .unwrap()
                .name
                .starts_with("Keycard · your flat")
        );
        // Waking again (a load) does not stack a second card.
        st.player = None;
        let _ = MapScreen::walking(&mut st);
        assert_eq!(st.character.count(&home_card), 1);
        let groups = crate::credentials::entries(&st);
        assert!(groups.iter().any(|(b, _, es)| {
            *b == h.building && es.iter().any(|e| e.text.contains("your own door"))
        }));
    }

    #[test]
    fn a_new_game_wakes_up_at_home() {
        let mut st = state_home();
        let s = MapScreen::walking(&mut st);
        let h = st.home.unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.place,
            Place::Floor {
                building: h.home.building,
                level: h.home.level
            }
        );
        assert_eq!(p.pos, h.home.bed_side);
        assert_eq!(s.place, p.place);
        assert_eq!(
            p.vis().get(h.home.rig),
            Some(&crate::walker::Visibility::Visible)
        );
    }

    #[test]
    fn a_city_without_a_home_starts_on_the_street() {
        let mut st = state_home();
        let mut city = st.city.take().unwrap();
        for b in city
            .buildings
            .iter_mut()
            .filter(|b| b.kind == citygen::BuildingKind::Megablock)
        {
            b.kind = citygen::BuildingKind::Apartment;
        }
        st.enter_city(city);
        assert!(st.home.is_none());
        MapScreen::walking(&mut st);
        assert!(matches!(
            st.player.as_ref().unwrap().place,
            Place::Stratum(StratumKind::Street)
        ));
    }

    #[test]
    fn the_home_door_opens_for_you_and_a_flats_stays_locked() {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        let h = st.home.unwrap().home;
        let place = Place::Floor {
            building: h.building,
            level: h.level,
        };
        let (_, floor) = st.interiors.floor(h.building, h.level).unwrap();
        let outside = h
            .door
            .neighbors4()
            .find(|n| !h.rect.contains(*n) && floor.tiles[*n].is_walkable())
            .expect("a walkway outside the home door");
        let flat = floor
            .rooms
            .iter()
            .filter(|r| r.private && r.kind == citygen::RoomKind::Flat)
            .find_map(|r| {
                let d = r.door?;
                let o = d
                    .neighbors4()
                    .find(|n| !r.rect.contains(*n) && floor.tiles[*n].is_walkable())?;
                Some((d, o))
            })
            .expect("a neighbouring flat");
        stand(&mut st, &mut s, place, outside);
        s.update(&mut st, &[key(dir_between(outside, h.door))]);
        assert_eq!(player(&st).1, h.door);
        assert_eq!(
            last(&st),
            Some(&*format!("Entering home · flat {}", h.number))
        );
        assert_eq!(st.log.latest().unwrap().marks.len(), 1, "home is lit");
        assert!(!last_is_warning(&st));
        let (door, o) = flat;
        stand(&mut st, &mut s, place, o);
        s.update(&mut st, &[key(dir_between(o, door))]);
        assert_eq!(player(&st).1, o);
        assert!(last(&st).is_some_and(|l| l.starts_with("locked · ")));
    }

    #[test]
    fn the_atrium_shows_the_plaza_below_and_nobody_on_it() {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        let h = st.home.unwrap().home;
        let place = Place::Floor {
            building: h.building,
            level: h.level,
        };
        let (_, floor) = st.interiors.floor(h.building, h.level).unwrap();
        let rail = floor
            .tiles
            .iter()
            .filter(|(_, t)| t.is_walkable())
            .map(|(p, _)| p)
            .find(|p| {
                p.neighbors4().any(|n| {
                    floor.tiles.get(n).map(|t| t.terrain) == Some(citygen::Terrain::Railing)
                })
            })
            .expect("a walkway beside the railing");
        stand(&mut st, &mut s, place, rail);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let area = s.last_map.get();
        let origin = s.cam.origin(area.width, area.height);
        let (_, floor) = st.interiors.floor(h.building, h.level).unwrap();
        let symbols: Vec<String> = (area.y..area.bottom())
            .flat_map(|sy| (area.x..area.right()).map(move |sx| (sx, sy)))
            .filter(|(sx, sy)| {
                let p = crate::render::viewport::screen_to_tile(area, origin, (*sx, *sy));
                floor.tiles.get(p).map(|t| t.terrain) == Some(citygen::Terrain::Atrium)
            })
            .map(|c| t.backend().buffer()[c].symbol().to_string())
            .collect();
        assert!(!symbols.is_empty(), "the atrium is on screen");
        assert!(
            symbols.iter().any(|c| c != " "),
            "the plaza shows through the atrium"
        );
        assert!(symbols.iter().all(|c| c != "@"), "nobody stands in the air");
    }

    /// The walkable tile of the home within reach of its rig, nearest the bed.
    fn beside_rig(st: &AppState) -> Pos {
        let h = st.home.unwrap().home;
        let (_, floor) = st.interiors.floor(h.building, h.level).unwrap();
        std::iter::once(h.rig)
            .chain(h.rig.neighbors8())
            .filter(|p| h.rect.contains(*p) && floor.tiles[*p].is_walkable())
            .min_by_key(|p| p.chebyshev(h.bed_side))
            .expect("a tile beside the rig")
    }

    #[test]
    fn space_at_a_street_terminal_offers_access_terminal() {
        let mut st = state8();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (jack, at) = {
            let city = st.city.as_ref().unwrap();
            let jack = city
                .connectors
                .iter()
                .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
                .unwrap()
                .pos;
            let at = jack
                .neighbors8()
                .find(|n| city.walkable(StratumKind::Street, *n))
                .unwrap();
            (jack, at)
        };
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(
            text.contains("Access terminal")
                && text.contains("i  Interact: Terminal")
                && text.contains("j  Jack in"),
            "{text}"
        );
        let row = text
            .lines()
            .find(|l| l.contains("Interact: Terminal"))
            .unwrap();
        let letter = row.chars().find(|c| c.is_ascii_alphabetic()).unwrap();
        let ScreenCmd::Replace { to: term, .. } = p.update(&mut st, &[key(KeyCode::Char(letter))])
        else {
            panic!("the row replaces the popup with the terminal")
        };
        st.tick += 10_000;
        let drawn = draw_screen(&*term, &st, 120, 40);
        assert!(
            drawn.contains("guest@") && drawn.contains(":/$ "),
            "{drawn}"
        );
        let _ = jack;
    }

    #[test]
    fn space_at_the_rig_offers_access_rig() {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        // A new game mails nothing now; the favour's mail is the one the rig reads.
        crate::jobs::first_favour(&mut st);
        let rig = st.home.unwrap();
        let home = Place::Floor {
            building: rig.home.building,
            level: rig.home.level,
        };
        let at = beside_rig(&st);
        stand(&mut st, &mut s, home, at);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(
            text.contains("Access rig") && text.contains("i  Interact: Terminal"),
            "{text}"
        );
        let row = text
            .lines()
            .find(|l| l.contains("Interact: Terminal"))
            .unwrap();
        let letter = row.chars().find(|c| c.is_ascii_alphabetic()).unwrap();
        let ScreenCmd::Replace { to: mut term, .. } =
            p.update(&mut st, &[key(KeyCode::Char(letter))])
        else {
            panic!("the row replaces the popup with the terminal")
        };
        st.tick += 10_000;
        let keys: Vec<_> = "cat ~/mail/001-a-favour.txt"
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain(std::iter::once(key(KeyCode::Enter)))
            .collect();
        term.update(&mut st, &keys);
        st.tick += 10_000;
        let drawn = draw_screen(&*term, &st, 120, 40);
        assert!(
            drawn.contains("ZENBO 64")
                && !drawn.contains("HOME RIG")
                && drawn.contains("@zenbo:~$ "),
            "{drawn}"
        );
        assert!(drawn.contains("SUBJECT  A FAVOUR"), "{drawn}");
    }

    #[test]
    fn jacking_in_from_the_home_rig_and_back() {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        let rig = st.home.unwrap();
        let home = Place::Floor {
            building: rig.home.building,
            level: rig.home.level,
        };
        let at = beside_rig(&st);
        stand(&mut st, &mut s, home, at);
        jack_in(&mut st, &mut s);
        assert_eq!(player(&st), (Place::Stratum(StratumKind::Net), rig.landing));
        assert_eq!(s.place, Place::Stratum(StratumKind::Net));
        assert_eq!(last(&st), Some("jacked in from home"));
        jack_out(&mut st, &mut s);
        assert_eq!(player(&st), (home, at));
        assert_eq!(s.place, home);
        assert_eq!(last(&st), Some("jacked out"));
        assert_eq!(
            st.player.as_ref().unwrap().vis().get(rig.home.rig),
            Some(&crate::walker::Visibility::Visible)
        );
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
        let lift = plan.lift_on(0).expect("a tower has a lift bank");
        let stair = plan.stair_on(0).expect("and a stairwell");
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
        assert!(text.contains("Floors"), "{text}");
        assert!(
            text.contains(" L ") || text.contains(" L·") || text.contains("L · here"),
            "{text}"
        );
        assert!(text.contains("2F"), "{text}");
        assert!(text.contains(&FloorPlan::label(plan.top())), "{text}");
        assert!(text.contains("roof"), "{text}");
        // The 2F row rides there, and the log says so.
        for _ in 0..80 {
            let text = draw_screen(&*popup, &st, 120, 60);
            if text
                .lines()
                .any(|l| l.contains("▸") && l.contains("Ride: 2F"))
            {
                break;
            }
            popup.update(&mut st, &[key(KeyCode::Down)]);
        }
        popup.update(&mut st, &[key(KeyCode::Enter)]);
        assert_eq!(last(&st), Some("Entering 2F"));
        s.update(&mut st, &[]);
        // A ride to a floor never generated before works (and generates it).
        assert!(st.interiors.floor(b, plan.top()).is_none());
        assert_eq!(
            crate::interiors::ride(&mut st, b, plan.top()).unwrap(),
            format!("Entering `{}`", FloorPlan::label(plan.top()))
        );
        s.update(&mut st, &[]);
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: plan.top()
            }
        );
        assert_eq!(pos, lift);
        assert!(st.interiors.floor(b, plan.top()).is_some());
        assert_eq!(s.place, place);
        // Stairs: one floor at a time, same tile.
        stand(&mut st, &mut s, place, stair);
        s.update(&mut st, &[key(KeyCode::Char('<'))]);
        assert_eq!(
            last(&st),
            Some(format!("Entering {}", FloorPlan::label(plan.top() - 1)).as_str())
        );
        assert_eq!(st.log.latest().unwrap().marks.len(), 1, "the floor is lit");
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: plan.top() - 1
            }
        );
        assert_eq!(pos, stair);
        s.update(&mut st, &[key(KeyCode::Char('>'))]);
        assert_eq!(
            player(&st).0,
            Place::Floor {
                building: b,
                level: plan.top()
            }
        );
        s.update(&mut st, &[key(KeyCode::Char('>'))]);
        assert_eq!(
            player(&st).0,
            Place::Floor {
                building: b,
                level: plan.top()
            },
            "no floor above the top"
        );
        assert!(
            last(&st).unwrap().contains("nothing leads up"),
            "{:?}",
            last(&st)
        );
        // Off the stair, > and < lead nowhere inside.
        let (_, floor) = st.interiors.floor(b, plan.top()).unwrap();
        let open = floor
            .tiles
            .iter()
            .find(|(p, t)| {
                t.terrain == citygen::Terrain::Floor && t.feature.is_none() && *p != stair
            })
            .unwrap()
            .0;
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: plan.top(),
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
                    StratumKind::Heights => plan.top(),
                    StratumKind::Skyways => plan.door_level(StratumKind::Skyways).unwrap(),
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
        assert_eq!(
            crate::net::jack_in_here(&mut st).unwrap_err(),
            "no terminal within reach"
        );
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let text = draw_screen(&*here, &st, 120, 40);
        assert!(!text.contains("Jack in"), "{text}");
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
        jack_in(&mut st, &mut s);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(p.pos, jack);
        assert_eq!(p.body, Some((Place::Stratum(StratumKind::Street), beside)));
        jack_out(&mut st, &mut s);
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
            .find(|b| matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment))
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
        assert!(
            popup.contains(&card.name) && popup.contains("t  Talk"),
            "{popup}"
        );
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
                    matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment)
                        && world
                            .inside(b.id)
                            .iter()
                            .any(|o| o.role == sim::OccupantRole::Living)
                })
                .expect("a residential block with people home at three")
                .id
        };
        let plan = st.interiors.plan(&city, b).unwrap().clone();
        st.interiors
            .ensure_homes(&city, st.world.as_ref().unwrap(), b);
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let mut found = None;
        for level in plan.corridor_floors() {
            st.interiors.ensure(&city, b, level).unwrap();
            let (plan, floor) = st.interiors.floor(b, level).unwrap();
            let placement =
                crate::interiors::Placement::new(plan, floor, st.world.as_ref().unwrap(), &homes);
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

    /// A residential building, a flat on one of its floors, its door, a tile outside it,
    /// the household and the owner's name.
    fn a_flat(
        st: &mut AppState,
    ) -> (
        citygen::BuildingId,
        i32,
        u16,
        Pos,
        Pos,
        sim::HouseholdId,
        String,
    ) {
        let city = st.city.as_ref().unwrap().clone();
        let b = {
            let world = st.world.as_ref().unwrap();
            city.buildings
                .iter()
                .find(|b| {
                    matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment)
                        && world.people().iter().any(|p| p.home == Some(b.id))
                })
                .expect("a residential block")
                .id
        };
        crate::credentials::prepare(st, b);
        let homes = st.interiors.homes_of(b).unwrap().clone();
        for (h, (level, number)) in homes.households() {
            let (_, floor) = st.interiors.floor(b, level).unwrap();
            let Some(r) = floor
                .rooms
                .iter()
                .find(|r| r.kind == citygen::RoomKind::Flat && r.number == number)
            else {
                continue;
            };
            let Some(d) = r.door else { continue };
            let Some(outside) = d
                .neighbors4()
                .find(|n| !r.rect.contains(*n) && floor.tiles[*n].is_walkable())
            else {
                continue;
            };
            let owner = homes.owner_of(level, number).unwrap();
            let name = st.world.as_ref().unwrap().people()[owner.0 as usize]
                .name
                .clone();
            return (b, level, number, d, outside, h, name);
        }
        panic!("a flat with a door and a corridor outside");
    }

    #[test]
    fn a_keycard_or_a_code_opens_a_locked_flat() {
        let mut st = state8();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (b, level, number, door, outside, h, name) = a_flat(&mut st);
        let place = Place::Floor { building: b, level };
        // Without anything: locked.
        stand(&mut st, &mut s, place, outside);
        s.update(&mut st, &[key(dir_between(outside, door))]);
        assert_eq!(player(&st).1, outside);
        assert!(
            last(&st).is_some_and(|l| l.starts_with(&format!("locked · {name}'s flat {number}"))),
            "{:?}",
            last(&st)
        );
        // With the code: in, and the log says you keyed it.
        let seed = st.city.as_ref().unwrap().seed;
        let code = citygen::rng::door_code(seed, b, level, number);
        st.journal.learn(crate::credentials::Code {
            building: b,
            level,
            number,
            household: Some(h),
            code,
            source: crate::credentials::Source::ServerCopy,
            noted_by: None,
            at: 0,
        });
        s.update(&mut st, &[key(dir_between(outside, door))]);
        assert_eq!(player(&st).1, door);
        assert_eq!(
            last(&st),
            Some(format!("{name}'s flat {number} · you key in {code:04}").as_str())
        );
        // Back out, forget the code, hold the card instead: the card wins.
        stand(&mut st, &mut s, place, outside);
        st.journal = Default::default();
        st.character.give(crate::credentials::keycard_id(b, h), 1);
        s.update(&mut st, &[key(dir_between(outside, door))]);
        assert_eq!(player(&st).1, door);
        assert_eq!(
            last(&st),
            Some(format!("{name}'s flat {number} · the keycard opens it").as_str())
        );
        // From inside the room the door opens whatever you hold.
        st.character.inventory.clear();
        let inside = {
            let (_, floor) = st.interiors.floor(b, level).unwrap();
            let r = floor.door_of(door).unwrap();
            door.neighbors4()
                .find(|n| r.rect.contains(*n) && floor.tiles[*n].is_walkable())
                .unwrap()
        };
        stand(&mut st, &mut s, place, inside);
        s.update(&mut st, &[key(dir_between(inside, door))]);
        assert_eq!(player(&st).1, door);
    }

    #[test]
    fn space_beside_a_locked_flat_offers_the_keypad_and_the_code_opens_the_door() {
        let mut st = state8();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (b, level, number, door, outside, _h, name) = a_flat(&mut st);
        let place = Place::Floor { building: b, level };
        stand(&mut st, &mut s, place, outside);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(
            text.contains(&format!("{name}'s flat {number}"))
                && text.contains("i  Interact: Keypad"),
            "{text}"
        );
        let row = text
            .lines()
            .find(|l| l.contains("Interact: Keypad"))
            .unwrap();
        let letter = row.chars().find(|c| c.is_ascii_alphabetic()).unwrap();
        let ScreenCmd::Replace { to: mut pad, .. } =
            p.update(&mut st, &[key(KeyCode::Char(letter))])
        else {
            panic!("the row replaces the popup with the keypad")
        };
        let code = citygen::rng::door_code(st.city.as_ref().unwrap().seed, b, level, number);
        let keys: Vec<_> = format!("{code:04}")
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain(std::iter::once(key(KeyCode::Char('v'))))
            .collect();
        pad.update(&mut st, &keys);
        assert!(draw_screen(&*pad, &st, 120, 40).contains("OPEN"));
        s.update(&mut st, &[key(dir_between(outside, door))]);
        assert_eq!(player(&st).1, door);
        assert!(
            last(&st).is_some_and(|l| l.contains("you key in")),
            "{:?}",
            last(&st)
        );
        // Inside the room, Space offers no keypad.
        let inside = {
            let (_, floor) = st.interiors.floor(b, level).unwrap();
            let r = floor.door_of(door).unwrap();
            door.neighbors4()
                .find(|n| r.rect.contains(*n) && floor.tiles[*n].is_walkable())
                .unwrap()
        };
        stand(&mut st, &mut s, place, inside);
        let ScreenCmd::Push { to: p2, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!()
        };
        assert!(!draw_screen(&*p2, &st, 120, 40).contains("keypad"));
    }

    #[test]
    fn your_own_door_has_no_keypad() {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        let h = st.home.unwrap().home;
        let outside = {
            let (_, floor) = st.interiors.floor(h.building, h.level).unwrap();
            h.door
                .neighbors4()
                .find(|n| !h.rect.contains(*n) && floor.tiles[*n].is_walkable())
                .unwrap()
        };
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: h.building,
                level: h.level,
            },
            outside,
        );
        let ScreenCmd::Push { to: p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!()
        };
        assert!(!draw_screen(&*p, &st, 120, 40).contains("keypad"));
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
            .find(|b| matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment))
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
                plan.lift_on(0).is_none() && plan.floors() >= 2
            })
            .expect("a two-floor building without a lift")
            .id;
        let plan = st.interiors.plan(&city, b).unwrap().clone();
        st.interiors.ensure(&city, b, plan.lowest()).unwrap();
        let stair = plan.stair_on(plan.lowest()).unwrap();
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: plan.lowest(),
            },
            stair,
        );
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let text = draw_screen(&*here, &st, 120, 40);
        assert!(
            text.contains(&format!(
                ">  Go up: {}",
                FloorPlan::label(plan.lowest() + 1)
            )),
            "{text}"
        );
        here.update(&mut st, &[key(KeyCode::Char('>'))]);
        s.update(&mut st, &[]);
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: plan.lowest() + 1
            }
        );
        assert_eq!(pos, stair, "stairs keep the tile");
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
        assert_eq!(last(&st), Some("Entering the GRID"));
        // In again: the construct is generated once; jacking out from inside works.
        s.update(&mut st, &[key(code)]);
        assert_eq!(st.player.as_ref().unwrap().place, Place::Construct(b));
        assert_eq!(st.constructs.generated(), 1);
        assert!(matches!(
            jack_out(&mut st, &mut s),
            ScreenCmd::Pop {
                fx: TransitionKind::Glitch
            }
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
        assert!(!text.contains("RUN") && text.contains("x  Look"), "{text}");
        assert!(text.contains("Crack: door lock · 40%"), "{text}");
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
        let lift = st.interiors.plan_of(b).unwrap().lift_on(1).unwrap();
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 1,
            },
            lift,
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
        assert_eq!(
            crate::net::jack_in_here(&mut st).unwrap_err(),
            "deck cooling · 10 min"
        );
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
            assert_eq!(
                crate::net::jack_in_here(&mut st).unwrap_err(),
                "port locked down"
            );
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
        assert!(st.backdoors.root_on(b), "{:?}", last(&st));
        let k = st.constructs.get(b).unwrap();
        assert!(!k.guards.is_empty());
        assert_eq!(k.guard_at(k.guards[0].pos), None, "the gate guard is down");
        let heat = st.net.traces[&b].heat;
        let now = st.net.now(&st);
        st.net
            .minute(now + 1, None, &mut st.constructs, &st.backdoors.roots());
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
        st.net.minute(
            now + game::ALERT_MINUTES + 1,
            None,
            &mut st.constructs,
            &st.backdoors.roots(),
        );
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
        assert!(
            !text.contains("Crack") && text.contains("x  Look"),
            "{text}"
        );
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
        jack_out(&mut st, &mut s);
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
        // And, for a building people live in, its door codes.
        let base = "server copied · a Paydata shard in the pack";
        if st.interiors.homes_of(b).is_some_and(|h| !h.is_empty()) {
            assert!(!st.journal.codes().is_empty());
            assert!(
                last(&st).is_some_and(
                    |l| l.starts_with(base) && l.ends_with("'s door codes are in the journal")
                ),
                "{:?}",
                last(&st)
            );
        } else {
            assert_eq!(last(&st), Some(base));
        }
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
            text.contains("Crack: door lock · 0% · 3 turns · noise 4"),
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
        // Calm, the run hangs off the lock on the map, not in the footer.
        assert!(
            text.contains("CRACK  40%") && !text.contains("RUN  crack"),
            "{text}"
        );
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
        assert!(
            st.fx
                .iter()
                .any(|i| matches!(i.fx, game::Fx::Number { .. }) && i.label == "+5"),
            "heat reads as a plus"
        );
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
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
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
        // The attack looks like a shot: a white streak, the tear, and the number as
        // a tag when it lands, a tick after the streak leaves.
        let number = st
            .fx
            .iter()
            .find(|i| matches!(i.fx, game::Fx::Number { tag: true }))
            .expect("a number for the attack");
        assert_eq!(
            (number.label.as_str(), number.tint, number.targets[0]),
            ("-2", Some(crate::net::ATTACK_TINT), gate)
        );
        assert_eq!(number.started, st.tick + 1);
        assert!(st.fx.iter().any(
            |i| matches!(i.fx, game::Fx::Streak { colour } if colour == crate::net::ATTACK_TINT)
        ));
        pass_turn(&mut st, &mut s);
        assert!(logged(&st, "shield absorbs the hit"));
        assert_eq!(st.character.focus, focus);
        assert_eq!(st.net.shield, 0);
        roll(&mut st, &mut s);
        assert_eq!(last(&st), Some("attack · grey ICE 2/6"));
        pass_turn(&mut st, &mut s);
        assert_eq!(st.character.focus, focus - 9);
        let drained = st
            .fx
            .iter()
            .rfind(|i| matches!(i.fx, game::Fx::Number { .. }) && i.label == "-9")
            .expect("a number for the hit on you");
        assert_eq!(drained.tint, Some(crate::net::BOLT_HIT));
        assert_eq!(drained.targets[0], st.player.as_ref().unwrap().pos);
        roll(&mut st, &mut s);
        assert_eq!(last(&st), Some("grey ICE broken"), "{:?}", last(&st));
        assert!(st.net.run.is_none());
        let k = st.constructs.get(b).unwrap();
        assert_eq!(k.guard_at(k.guards[0].pos), None);
        assert!(k.tiles[k.guards[0].pos].is_walkable());
        let now = st.net.now(&st);
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
        assert_eq!(
            st.constructs.get(b).unwrap().guard_at(post),
            None,
            "stays down"
        );
        // After the hour it stands where it was, whole again.
        st.net.minute(
            now + game::HACK_MINUTES,
            None,
            &mut st.constructs,
            &st.backdoors.roots(),
        );
        let k = st.constructs.get(b).unwrap();
        assert_eq!(k.guard_at(k.guards[0].pos), Some(0));
        advance(&mut st, &mut s);
        assert_eq!(st.net.ice[&b][0].integrity, 6, "whole again");
        // The piece stood again awake, so the beat put the board on turns: stay
        // first. Then the RUN popup offers the deck programs on yourself.
        if crate::net::phase(&st) == Some(crate::net::Phase::Move) {
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
        }
        // The fire cursor is up on the piece; esc drops it so space opens the
        // popup for the programs on yourself.
        if s.target.is_some() {
            s.update(&mut st, &[key(KeyCode::Esc)]);
        }
        let ScreenCmd::Push { to: run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!("space opens RUN")
        };
        st.tick += 40;
        let text = draw_screen(&*run, &st, 120, 40);
        assert!(text.contains("Shield: yourself · 100% · 1 turn"), "{text}");
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
        jack_out(&mut st, &mut s);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert_eq!(st.character.focus, focus, "{:?}", last(&st));
        assert!(logged(&st, "jacked out"));
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

    /// Seed 8 walked into with a run saved in `dir`, standing in the flat: the state
    /// and the run's file.
    fn saved_at_home(
        dir: &crate::runs::testutil::Dir,
        perma: bool,
    ) -> (AppState, std::path::PathBuf) {
        let mut st = state_home();
        let _ = MapScreen::walking(&mut st);
        let mut st = crate::runs::testutil::running(st);
        st.saves = Some(dir.0.clone());
        st.perma_death = perma;
        st.run.as_mut().unwrap().born = st.net.now(&st);
        assert!(crate::runs::when::save_now(&mut st));
        let path = dir.0.join(format!("{}.run", st.run.unwrap().id));
        (st, path)
    }

    /// Black ICE kills you in a tower's construct, the failsafe off; the cause, and the
    /// screen the map's next frame switches to, a breath on.
    fn killed_on_the_net(st: &mut AppState) -> (String, Box<dyn Screen>) {
        let mut s = MapScreen::explore(st.tick);
        let b = in_construct_at(st, &mut s, 230);
        let name = crate::net::building_name(st, b);
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        assert_eq!(vault_guard.grade, IceGrade::Black);
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        st.net.failsafe = false;
        st.character.hp = 4;
        stand(st, &mut s, Place::Construct(b), beside_vault);
        assert_eq!(st.character.hp, 0);
        let ScreenCmd::Switch { to: dead, .. } = s.update(st, &[]) else {
            panic!("death switches the screen")
        };
        st.tick += 40;
        (format!("black ICE at {name}"), dead)
    }

    #[test]
    fn a_perma_flatline_goes_to_the_hall() {
        use crate::runs::{self, file};
        let dir = crate::runs::testutil::Dir::new("perma-flatline");
        let (mut st, path) = saved_at_home(&dir, true);
        let run = st.run.unwrap();
        let saved = file::header(&path).unwrap();
        advance_minutes(&mut st, 90);
        let (cause, mut dead) = killed_on_the_net(&mut st);
        // The file stays, its header marked with the death.
        let h = file::header(&path).expect("the run's file stays");
        let d = h.death.clone().expect("marked with the death");
        assert_eq!(d.cause, cause);
        assert_eq!(
            (d.day, d.minute),
            (crate::ui::hud::day_at(st.clock_tick()), st.minute_of_day())
        );
        assert_ne!(
            (d.day, d.minute),
            (saved.day, saved.minute),
            "its own clock"
        );
        assert_eq!(d.lasted, st.net.now(&st) - run.born);
        assert!(d.lasted >= 90, "{d:?}");
        assert!(d.at >= saved.saved && d.at == h.saved, "{d:?}");
        assert_eq!(h.place, "on the Net", "saved as it ended");
        assert!(st.run.is_none(), "the run is over");
        assert!(runs::living(&dir.0).is_empty(), "out of LOAD GAME");
        let hall = runs::dead(&dir.0);
        assert_eq!(hall.len(), 1);
        assert_eq!(hall[0].path, path);
        assert!(runs::newest(&dir.0).is_none(), "no CONTINUE");
        assert!(runs::open(&mut AppState::new(), &path).is_err());
        // The screen taunts, one line picked by the run, and says nothing of the hall.
        let text = draw_screen(&*dead, &st, 120, 40);
        let taunts = crate::screens::flatlined::Taunts::load();
        let taunt = taunts.pick(run.id);
        assert!(text.contains(taunt), "{text}");
        assert_eq!(
            taunts
                .all()
                .iter()
                .filter(|t| text.contains(t.as_str()))
                .count(),
            1
        );
        assert!(text.contains(&cause) && text.contains("VEX · level 1"));
        assert!(text.contains("any key · title"), "{text}");
        assert!(!text.contains("HALL OF FAME"), "{text}");
        // Any key: the title; nothing writes the run again.
        let bytes = std::fs::read(&path).unwrap();
        assert!(matches!(
            dead.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        assert!(!crate::runs::when::save_now(&mut st));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        // The HALL OF FAME has it.
        let hall = crate::screens::runs::RunsScreen::hall(&st);
        let text = draw_screen(&hall, &st, 120, 40);
        assert!(text.contains("VEX") && text.contains("FLATLINED"), "{text}");
    }

    #[test]
    fn a_flatline_without_perma_reloads() {
        use crate::runs::{describe, file};
        let dir = crate::runs::testutil::Dir::new("reload-flatline");
        let (mut st, path) = saved_at_home(&dir, false);
        let saved = file::header(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let home = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        let Place::Floor { level, .. } = home.0 else {
            panic!("saved in the flat: {home:?}")
        };
        let credits = st.character.credits;
        st.character.credits += 500;
        advance_minutes(&mut st, 25);
        let (_, mut dead) = killed_on_the_net(&mut st);
        assert_ne!(
            st.minute_of_day(),
            saved.minute,
            "it died later than it saved"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "nothing is marked");
        assert!(st.run.is_some());
        // The line is the save's own: its clock, and in the flat just `at home`.
        assert!(saved.place.ends_with(" · at home"), "{}", saved.place);
        assert_eq!(describe::clock(saved.day, saved.minute), "day 1 22:00");
        let text = draw_screen(&*dead, &st, 120, 40);
        let row = text
            .lines()
            .find(|l| l.contains("reload last save point"))
            .unwrap_or_else(|| panic!("{text}"));
        assert_eq!(row.trim(), "reload last save point · day 1 22:00 · at home");
        assert!(
            text.contains("space · load last save     esc · title"),
            "{text}"
        );
        assert!(!text.contains("any key"));
        // Space: back in the flat as it was saved, straight onto the map.
        let ScreenCmd::Switch { to: map, .. } = dead.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space loads the last save")
        };
        assert!(st.net.death.is_none());
        assert_eq!(st.character.credits, credits, "the saved credits");
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), home);
        assert!(!p.jacked_in());
        assert_eq!(st.run.map(|r| r.id), Some(saved.id));
        let text = draw_screen(&*map, &st, 120, 40);
        assert!(
            text.contains(&format!(" on foot · {}", FloorPlan::label(level))),
            "the map draws the flat\n{text}"
        );
        assert!(!text.contains("JACK IN"), "no broadcast");
    }

    #[test]
    fn a_run_saved_in_a_construct_opens_there() {
        let dir = crate::runs::testutil::Dir::new("resume-construct");
        let (mut st, path) = saved_at_home(&dir, false);
        let mut s = MapScreen::explore(st.tick);
        let b = in_construct(&mut st, &mut s);
        assert!(crate::runs::when::save_now(&mut st));
        let at = st.player.as_ref().map(|p| (p.place, p.pos, p.body));
        let mut back = AppState::new();
        crate::runs::open(&mut back, &path).unwrap();
        let map = MapScreen::resumed(&mut back);
        assert_eq!(map.place, Place::Construct(b));
        assert_eq!(back.player.as_ref().map(|p| (p.place, p.pos, p.body)), at);
        let text = draw_screen(&map, &back, 120, 40);
        let name = crate::net::building_name(&back, b);
        let footer = text
            .lines()
            .find(|l| l.starts_with(" ⚡"))
            .unwrap_or_else(|| panic!("jacked in, the footer says so\n{text}"));
        assert!(
            footer
                .trim_start_matches([' ', '⚡'])
                .starts_with(&format!("{name} · ")),
            "the map draws the construct: {footer:?}"
        );
        assert!(!text.contains(" on foot · "), "{text}");
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
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
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
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
        let spot = st.constructs.get(b).unwrap().guards[0].pos;
        st.constructs.get_mut(b).unwrap().move_guard(1, spot);
        st.net.minute(
            now + game::HACK_MINUTES,
            None,
            &mut st.constructs,
            &st.backdoors.roots(),
        );
        let k = st.constructs.get(b).unwrap();
        assert_eq!(
            k.guard_at(spot),
            Some(1),
            "the standing piece keeps the tile"
        );
        assert_eq!(k.guards[0].pos, spot);
        let away = k.guards[1].post;
        st.constructs.get_mut(b).unwrap().move_guard(1, away);
        st.net.sync_guards(
            &mut st.constructs,
            b,
            now + game::HACK_MINUTES,
            &st.backdoors.roots(),
        );
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

    /// A lane tile of the Net from which a step east rides three tiles or more, and
    /// the ride.
    fn a_lane_to_ride(st: &AppState) -> (Pos, Vec<Pos>) {
        let city = st.city.as_ref().unwrap();
        let net = city.stratum(StratumKind::Net).unwrap();
        let map = MapRef::stratum_of(city, StratumKind::Net);
        net.tiles
            .iter()
            .filter(|(_, t)| t.terrain == citygen::Terrain::NetLane)
            .find_map(|(p, _)| {
                let path = crate::ride::ride_path(&map, p, Dir::E);
                (path.len() >= 3).then_some((p, path))
            })
            .expect("a lane to ride")
    }

    fn on_a_lane(st: &mut AppState, s: &mut MapScreen) -> Vec<Pos> {
        walking(st, s);
        // Riding is the Data surfer's.
        st.character
            .talents
            .insert(game::TalentId::new("data_surfer"), 1);
        let (from, path) = a_lane_to_ride(st);
        st.player.as_mut().unwrap().body = Some((Place::Stratum(StratumKind::Street), from));
        stand(st, s, Place::Stratum(StratumKind::Net), from);
        path
    }

    #[test]
    fn you_are_blue_on_the_net() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        on_a_lane(&mut st, &mut s);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let you = crate::render::netlook::YOU.to_color();
        let cells: Vec<_> = buf
            .content()
            .iter()
            .filter(|c| c.symbol() == "@" && c.fg == you)
            .collect();
        assert_eq!(cells.len(), 1, "one blue @");
        assert_eq!(cells[0].bg, crate::render::netlook::YOU_GLOW.to_color());
    }

    #[test]
    fn a_step_along_a_lane_rides_to_its_stop() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let path = on_a_lane(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(
            st.player.as_ref().unwrap().pos,
            path[0],
            "the first tile at once"
        );
        assert!(s.riding.is_some());
        for _ in 0..path.len() + 2 {
            st.tick += crate::ride::RIDE_TICKS;
            s.update(&mut st, &[]);
        }
        assert_eq!(st.player.as_ref().unwrap().pos, *path.last().unwrap());
        assert!(s.riding.is_none(), "it ends at its stop");
    }

    #[test]
    fn without_data_surfer_a_lane_is_walked() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let path = on_a_lane(&mut st, &mut s);
        st.character
            .talents
            .remove(&game::TalentId::new("data_surfer"));
        let from = st.player.as_ref().unwrap().pos;
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(st.player.as_ref().unwrap().pos, path[0], "one step");
        assert!(s.riding.is_none(), "no ride");
        for _ in 0..4 {
            st.tick += crate::ride::RIDE_TICKS;
            s.update(&mut st, &[]);
        }
        assert_eq!(
            st.player.as_ref().unwrap().pos,
            path[0],
            "and it stays there"
        );
        assert_ne!(from, path[0]);
    }

    #[test]
    fn slipstream_sheds_trace_while_riding() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let path = on_a_lane(&mut st, &mut s);
        let b = st.city.as_ref().unwrap().buildings[0].id;
        st.net.trace(b).heat = 50;
        // Without Slipstream nothing sheds, whatever the roll.
        st.net.force_slip = Some(true);
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        for _ in 0..path.len() + 2 {
            st.tick += crate::ride::RIDE_TICKS;
            s.update(&mut st, &[]);
        }
        assert_eq!(st.net.traces[&b].heat, 50);
        // With it, a tile that rolls sheds a tenth of every trace on you.
        st.character
            .talents
            .insert(game::TalentId::new("slipstream"), 1);
        let at = st.player.as_ref().unwrap().pos;
        assert!(crate::ride::slipstream(&mut st, at));
        assert_eq!(st.net.traces[&b].heat, 45);
        assert_eq!(
            last(&st),
            Some("slipstream · the trace loses a tenth of you")
        );
        st.net.force_slip = Some(false);
        assert!(!crate::ride::slipstream(&mut st, at));
        assert_eq!(st.net.traces[&b].heat, 45);
        // Unforced, about one tile in twenty sheds.
        st.net.force_slip = None;
        let hits = (0..2000)
            .filter(|i| {
                st.net.trace(b).heat = 200;
                crate::ride::slipstream(&mut st, Pos::new(at.x + i % 40, at.y + i / 40))
            })
            .count();
        assert!((40..=170).contains(&hits), "{hits} of 2000");
    }

    #[test]
    fn any_key_stops_a_ride() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let path = on_a_lane(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        let at = st.player.as_ref().unwrap().pos;
        assert_eq!(at, path[0]);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert!(s.riding.is_none());
        assert!(s.looking.is_none(), "the key did nothing else");
        for _ in 0..5 {
            st.tick += 1;
            s.update(&mut st, &[]);
        }
        assert_eq!(st.player.as_ref().unwrap().pos, at);
    }

    #[test]
    fn no_ride_inside_a_construct() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        in_construct(&mut st, &mut s);
        for code in ['h', 'j', 'k', 'l'] {
            s.update(&mut st, &[key(KeyCode::Char(code))]);
            assert!(s.riding.is_none());
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
        let lines = crate::net::ice_lines(&st, Place::Construct(b), |_| true);
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
    fn the_fire_cursor_skips_ice_behind_a_wall() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        // A room tile some standing piece has no clear line to.
        let hide = {
            let k = st.constructs.get(b).unwrap();
            k.rooms
                .iter()
                .flat_map(|r| r.iter())
                .find(|p| {
                    k.tiles[*p].is_walkable()
                        && k.guards.iter().any(|g| {
                            k.guard_at(g.pos).is_some() && !crate::net::line_clear_in(k, *p, g.pos)
                        })
                })
                .expect("a room tile a wall hides from a piece")
        };
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let k = st.constructs.get(b).unwrap();
        let standing: Vec<Pos> = k
            .guards
            .iter()
            .filter(|g| k.guard_at(g.pos).is_some())
            .map(|g| g.pos)
            .collect();
        let walled = standing
            .iter()
            .filter(|g| !crate::net::line_clear_in(k, hide, **g))
            .count();
        assert!(walled > 0, "a piece behind a wall from {hide:?}");
        let picks = MapScreen::enemies_in_sight(&st);
        for (_, at) in &picks {
            assert!(
                crate::net::line_clear_in(k, hide, *at),
                "{at:?} is behind a wall"
            );
        }
        assert_eq!(picks.len() + walled, standing.len());
    }

    #[test]
    fn signal_ghost_marks_where_awake_ice_steps() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let k = st.constructs.get(b).unwrap().clone();
        st.net.ice_of(&k);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        assert!(
            crate::net::ghost_steps(&st, b).is_empty(),
            "asleep: nothing"
        );
        st.net.wake_all(b, hide);
        let steps = crate::net::ghost_steps(&st, b);
        assert!(!steps.is_empty(), "awake: a step each");
        let now = st.constructs.get(b).unwrap();
        for (p, _) in &steps {
            assert!(
                now.guards.iter().any(|g| g.pos.chebyshev(*p) == 1),
                "a step beside its piece: {p:?}"
            );
        }
        // A stunned piece loses its turn: no mark.
        for ice in st.net.ice.get_mut(&b).unwrap().iter_mut() {
            ice.stunned = 1;
        }
        assert!(
            crate::net::ghost_steps(&st, b).is_empty(),
            "stunned: nothing"
        );
        for ice in st.net.ice.get_mut(&b).unwrap().iter_mut() {
            ice.stunned = 0;
        }
        // Everything in view: the lines list both pieces however the walls lie.
        let lines = crate::net::ice_lines(&st, Place::Construct(b), |_| true);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines.iter().all(|l| l.contains("hunting")));
        // Without the ability, no steps.
        st.character =
            game::Character::from_template(&st.content, &game::TemplateId::new("kessler")).unwrap();
        assert!(crate::net::ghost_steps(&st, b).is_empty());
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
            .chrome
            .iter()
            .find(|(_, id)| id.0 == "nightglass_optics")
            .map(|(site, _)| *site)
            .unwrap();
        st.character.chrome.remove(&eyes);
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

    /// Seed 8 on the Net beside a street terminal of Kiroflux Mart's district, held.
    fn street_terminal_held() -> (AppState, MapScreen) {
        let (mut st, b) = crate::net::tests::under_kiroflux();
        let mut s = MapScreen::explore(0);
        let city = st.city.as_ref().unwrap();
        let d = city.buildings[b.index()].district;
        let owner = crate::owners::Owner::District(d);
        let term = city
            .net
            .nodes
            .iter()
            .find(|n| {
                n.kind == citygen::NodeKind::Terminal
                    && crate::owners::owner_of(city, n.id) == Some(owner)
            })
            .expect("a street terminal");
        let (id, at) = (term.id, term.cyber);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), at);
        let now = st.net.now(&st);
        crate::holds::take(&mut st, citygen::NetNode::City(id), owner, now);
        (st, s)
    }

    /// A grey grid piece hunting you, at `at`.
    fn grid_piece(st: &AppState, at: Pos) -> crate::foes::Foe {
        let d = st.city.as_ref().unwrap().districts[0].id;
        let me = st.player.as_ref().unwrap().pos;
        let mut f = crate::grid::piece(
            citygen::IceGrade::Grey,
            crate::grid::Role::Sentinel {
                beat: vec![],
                step: 0,
                forward: true,
            },
            crate::owners::Owner::District(d),
            at,
        );
        f.mode = crate::foes::FoeMode::Hunting { seen: me, lost: 0 };
        f
    }

    /// You on the Net (seed 8, sweeps so no sentinels) at the west end of a straight
    /// lane four tiles long.
    fn on_a_straight_lane() -> (AppState, MapScreen, Pos) {
        let (mut st, _) = crate::net::tests::under_kiroflux();
        st.grid_patrols = crate::grid::Patrols::Sweeps;
        let mut s = MapScreen::explore(0);
        let city = st.city.clone().unwrap();
        let me = city
            .stratum(StratumKind::Net)
            .unwrap()
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| {
                (0..=3).all(|k| {
                    city.tile(StratumKind::Net, Pos::new(p.x + k, p.y))
                        .is_some_and(|t| t.terrain == citygen::Terrain::NetLane)
                })
            })
            .unwrap();
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), me);
        (st, s, me)
    }

    #[test]
    fn space_beside_a_camera_on_the_net_cracks_it_then_offers_its_verbs() {
        let (mut st, b) = crate::net::tests::under_kiroflux();
        st.grid_patrols = crate::grid::Patrols::Sweeps;
        let mut s = MapScreen::explore(0);
        let (cam, at) = crate::net::tests::node_cyber(&st, b, citygen::NodeKind::Camera);
        stand(
            &mut st,
            &mut s,
            Place::Stratum(StratumKind::Net),
            Pos::new(at.x, at.y + 1),
        );
        // Space: the programs that reach what's beside you; `a` is Crack on the camera.
        let ScreenCmd::Push { to: mut w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("a window of what the deck reaches");
        };
        st.net.force_roll = Some(10);
        w.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert_eq!(
            st.net.run.as_ref().map(|r| r.target),
            Some(game::HackTarget::Node(cam)),
            "{:?}",
            last(&st)
        );
        // Space hurries the beats until it lands.
        for _ in 0..6 {
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
        }
        let now = st.net.now(&st);
        assert!(
            crate::holds::held(&st, citygen::NetNode::City(cam), now).is_some(),
            "{:?}",
            last(&st)
        );
        // Space again: the held camera's verbs on their own keys.
        let ScreenCmd::Push { to: mut w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("the window with the camera's verbs");
        };
        w.update(&mut st, &[key(KeyCode::Char('o'))]);
        assert!(
            crate::holds::looped(&st, citygen::NetNode::City(cam), now),
            "{:?}",
            last(&st)
        );
        let ScreenCmd::Push { to: mut w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("the window again");
        };
        w.update(&mut st, &[key(KeyCode::Char('f'))]);
        assert_eq!(
            st.net.feed,
            Some(citygen::NetNode::City(cam)),
            "the feed opens from the window too"
        );
    }

    #[test]
    fn a_move_or_esc_closes_the_feed() {
        let (mut st, b) = crate::net::tests::under_kiroflux();
        st.grid_patrols = crate::grid::Patrols::Sweeps;
        let mut s = MapScreen::explore(0);
        let (cam, at) = crate::net::tests::node_cyber(&st, b, citygen::NodeKind::Camera);
        let here = Pos::new(at.x, at.y + 1);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), here);
        let now = st.net.now(&st);
        crate::holds::take(
            &mut st,
            citygen::NetNode::City(cam),
            crate::owners::Owner::Building(b),
            now,
        );
        // Esc closes it, and doesn't open the menu.
        s.update(&mut st, &[key(KeyCode::Char('f'))]);
        assert_eq!(st.net.feed, Some(citygen::NetNode::City(cam)));
        let cmd = s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(matches!(cmd, ScreenCmd::Stay), "no menu while a feed is up");
        assert_eq!(st.net.feed, None);
        assert_eq!(last(&st), Some("feed closed"));
        // With no feed, Esc is the menu again.
        let cmd = s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(matches!(cmd, ScreenCmd::Push { .. }));
        // A step closes it.
        s.update(&mut st, &[key(KeyCode::Char('f'))]);
        assert_eq!(st.net.feed, Some(citygen::NetNode::City(cam)));
        let city = st.city.clone().unwrap();
        let step = [Dir::E, Dir::W, Dir::S, Dir::N]
            .into_iter()
            .find(|d| city.walkable(StratumKind::Net, here.step(*d)))
            .expect("somewhere to step");
        s.update(&mut st, &[key(dir_key(step))]);
        assert_ne!(st.player.as_ref().unwrap().pos, here, "moved");
        assert_eq!(st.net.feed, None, "moving off closes the feed");
    }

    #[test]
    fn far_grid_ice_hunting_never_puts_you_on_turns() {
        let (mut st, mut s, me) = on_a_straight_lane();
        let far = grid_piece(&st, Pos::new(me.x + 40, me.y));
        st.foes.push(far);
        for _ in 0..4 {
            s.update(&mut st, &[]);
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
            assert!(st.net.engaged.is_none(), "{:?}", last(&st));
        }
    }

    #[test]
    fn a_fight_on_the_open_net_takes_the_decks_attack() {
        let (mut st, mut s, me) = on_a_straight_lane();
        let attack = game::ItemId::new("attack");
        if !st.character.loaded.contains(&attack) {
            st.character.loaded.insert(0, attack);
        }
        let piece = st.foes.len();
        let f = grid_piece(&st, Pos::new(me.x + 3, me.y));
        st.foes.push(f);
        crate::net::engage(&mut st, Place::Stratum(StratumKind::Net), false);
        settle(&mut st, &mut s);
        assert!(crate::net::your_turn(&st));
        // Stay: the act half; the cursor on the piece; space attacks with the deck.
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        st.net.force_roll = Some(10);
        s.target = Some(st.foes[piece].pos);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(
            st.foes[piece].hp < st.foes[piece].hp_max,
            "the attack lands: {:?}",
            last(&st)
        );
        // No meat on the Net: nothing to throw, the programs' keys first.
        assert_eq!(MapScreen::act_base(&st), 6);
    }

    #[test]
    fn a_ride_aborts_the_run_and_a_mask_wears_on_the_grid() {
        let (mut st, _) = crate::net::tests::under_kiroflux();
        st.grid_patrols = crate::grid::Patrols::Sweeps;
        let mut s = MapScreen::explore(0);
        let city = st.city.clone().unwrap();
        // A wall camera with a lane to ride off from beside it.
        let (cam, here, dir) = city
            .net
            .nodes
            .iter()
            .filter(|n| n.kind == citygen::NodeKind::Camera && n.building.is_some())
            .find_map(|n| {
                n.cyber.neighbors8().find_map(|q| {
                    let reach = crate::net::targets_outside(&st, q)
                        .iter()
                        .any(|(t, _)| *t == game::HackTarget::Node(n.id));
                    [Dir::E, Dir::W, Dir::S, Dir::N]
                        .into_iter()
                        .find(|d| {
                            let map = MapRef::stratum_of(&city, StratumKind::Net);
                            reach && crate::ride::ride_path(&map, q, *d).len() >= 3
                        })
                        .map(|d| (n.id, q, d))
                })
            })
            .expect("a lane to ride beside a wall camera");
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), here);
        crate::net::start_run(
            &mut st,
            game::ItemId::new("crack"),
            game::HackTarget::Node(cam),
        )
        .unwrap();
        s.update(&mut st, &[key(dir_key(dir))]);
        assert!(st.net.run.is_none(), "riding off drops the run");
        st.net.mask = 3;
        for _ in 0..(4 * crate::net::BEAT_TICKS) {
            st.tick += 1;
            if let (Some(c), Some(w)) = (st.city.as_ref(), st.world.as_mut()) {
                w.advance(c, w.tick() + 1);
            }
            s.update(&mut st, &[]);
        }
        assert_eq!(st.net.mask, 0, "the mask wore off");
    }

    #[test]
    fn shell_opens_a_held_terminal_from_the_net() {
        let (mut st, mut s) = street_terminal_held();
        let cmd = s.update(&mut st, &[key(KeyCode::Char('r'))]);
        assert!(matches!(cmd, ScreenCmd::Push { .. }), "the terminal opens");
        // A key no held node answers says so.
        let cmd = s.update(&mut st, &[key(KeyCode::Char('f'))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(last(&st), Some("the terminal has nothing on [f]"));
    }

    #[test]
    fn the_dock_draws_on_the_held_node() {
        let (st, s) = street_terminal_held();
        let text = draw_screen(&s, &st, 180, 40);
        assert!(text.contains("TERMINAL  HELD 60m"), "{text}");
        assert!(text.contains("[r] shell"));
    }

    #[test]
    fn hijack_opens_the_picker_and_puts_the_line_up() {
        let (mut st, b) = crate::net::tests::under_kiroflux();
        let mut s = MapScreen::explore(0);
        let (ad, at) = crate::net::tests::node_cyber(&st, b, citygen::NodeKind::AdBoard);
        stand(
            &mut st,
            &mut s,
            Place::Stratum(StratumKind::Net),
            Pos::new(at.x, at.y - 1),
        );
        let now = st.net.now(&st);
        crate::holds::take(
            &mut st,
            citygen::NetNode::City(ad),
            crate::owners::Owner::Building(b),
            now,
        );
        let ScreenCmd::Push { to: mut picker, .. } = s.update(&mut st, &[key(KeyCode::Char('i'))])
        else {
            panic!("the hijack's picker");
        };
        picker.update(&mut st, &[key(KeyCode::Char('s'))]);
        assert!(
            last(&st).is_some_and(|l| l.contains("KIROFLUX MART SELLS YOUR FACE")),
            "{:?}",
            last(&st)
        );
        assert!(!crate::holds::hijacked_ads(&st).is_empty());
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
        // On the grid a program reaches what is beside you: here, nothing.
        let cmd = s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(last(&st), Some("crack · nothing to run it against"));
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
        assert!(
            text.contains("CRACK  40%") && !text.contains("RUN  crack"),
            "{text}"
        );
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
        assert!(
            !text.contains("RUN  crack") && !text.contains("CRACK  40%"),
            "{text}"
        );
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
        let r = layout(Rect::new(0, 0, 100, 30), false);
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
        // Jacked in with a calm run: it hangs on the map; the same four rows.
        let (_, _) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert!(st.net.run.is_some());
        let text = draw_screen(&s, &st, 120, 40);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[36].starts_with(" ⚡ "), "{:?}", lines[36]);
        assert!(lines[36].contains("TRACE ▱▱▱▱▱▱▱▱▱▱ 4"), "{:?}", lines[36]);
        assert_eq!(lines[37].trim(), "");
        assert!(!lines[38].contains("RUN"), "{:?}", lines[38]);
        assert!(
            lines[39].starts_with(" PROGRAMS   [1] CRACK ●    [2] COPY ●"),
            "{:?}",
            lines[39]
        );
        // The console takes the last row.
        st.unlock_dev();
        s.update(&mut st, &[key(KeyCode::Char(':'))]);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.lines().nth(39).unwrap().contains(":"), "{text}");
        assert!(text.contains("CRACK  40%"), "{text}");
    }

    #[test]
    fn help_lists_the_keys_the_footer_dropped() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        let text = draw_screen(&s, &st, 140, 45);
        for k in [
            "space",
            "enter",
            "tab",
            "v ",
            "m ",
            ". ",
            "J ",
            "jack out (asks first)",
            "x ",
            "look: a cursor",
        ] {
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
    fn the_pulse_rides_toward_the_next_roll() {
        use crate::net::{BEAT_TICKS, ROLL_TICKS, beat_phase, tether_view};
        use crate::render::tether::TetherState;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (_, lock) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(beat_phase(&st), 0.0);
        st.tick += BEAT_TICKS / 2;
        assert!((beat_phase(&st) - 0.5).abs() < 0.02, "{}", beat_phase(&st));
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        let view = tether_view(&st, s.place).expect("a calm run has a tether");
        let lock_pos = st
            .constructs
            .get(match s.place {
                Place::Construct(b) => b,
                _ => unreachable!(),
            })
            .unwrap()
            .nodes
            .iter()
            .find(|(id, _)| *id == lock)
            .unwrap()
            .1;
        assert_eq!(view.target, lock_pos);
        assert_eq!((view.turns, view.work, view.odds), (1, 3, 40));
        assert_eq!(
            (view.op.as_str(), view.name.as_str()),
            ("CRACK", "door lock")
        );
        // The hit flashes, then the pulse rides on.
        assert_eq!(view.state, TetherState::Hit);
        st.tick += ROLL_TICKS;
        let view = tether_view(&st, s.place).unwrap();
        assert_eq!(view.state, TetherState::Running);
        assert!(view.phase > 0.5, "{}", view.phase);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(!text.contains("next roll"), "{text}");
    }

    #[test]
    fn a_landed_run_holds_its_tether_for_a_beat() {
        use crate::net::tether_view;
        use crate::render::tether::{LAND_TICKS, TetherState};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (_, lock) = lock_run(&mut st, &mut s);
        crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(logged(&st, "lock cracked"), "{:?}", last(&st));
        assert!(st.net.run.is_none());
        let view = tether_view(&st, s.place).expect("the landing's beat");
        assert_eq!(view.state, TetherState::Landed { age: 0 });
        // Past the landing's kick, which glitches the whole map for a moment.
        st.tick += 8;
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("CRACKED"), "{text}");
        st.tick += LAND_TICKS - 8;
        assert!(tether_view(&st, s.place).is_none());
    }

    #[test]
    fn a_slip_shows_on_the_tether() {
        use crate::net::{MARK_TICKS, tether_view};
        use crate::render::tether::TetherState;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (_, lock) = lock_run(&mut st, &mut s);
        st.net.force_roll = Some(1);
        crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(logged(&st, "crack slips"), "{:?}", last(&st));
        assert_eq!(tether_view(&st, s.place).unwrap().state, TetherState::Slip);
        st.tick += MARK_TICKS;
        assert_eq!(
            tether_view(&st, s.place).unwrap().state,
            TetherState::Running
        );
    }

    #[test]
    fn a_calm_run_with_no_tether_stays_in_the_footer() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, _) = lock_run(&mut st, &mut s);
        // A run left on a piece after a fight ends: no tether for ICE.
        st.net.run = Some(game::Run {
            program: ItemId::new("crack"),
            target: game::HackTarget::Guard {
                building: b,
                index: 0,
            },
            done: 1,
            work: 3,
        });
        assert!(st.net.engaged.is_none());
        assert!(crate::net::tether_view(&st, s.place).is_none());
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("RUN  crack · grey ICE  ▰▱▱ 1/3"), "{text}");
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

    #[test]
    fn x_looks_and_esc_or_x_puts_it_away() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert_eq!(s.looking.as_ref().map(|l| l.cursor), Some(at));
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(s.looking.as_ref().unwrap().cursor, at.offset(1, 0));
        assert_eq!(player(&st).1, at, "looking never moves you");
        s.update(&mut st, &[key(KeyCode::Char('L'))]);
        assert_eq!(
            s.looking.as_ref().unwrap().cursor,
            at.offset(9, 0),
            "shift moves eight"
        );
        st.tick += 40;
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("LOOK") && text.contains("esc back"), "{text}");
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(s.looking.is_none(), "esc puts it away");
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert!(s.looking.is_none(), "x again puts it away");
    }

    #[test]
    fn looking_at_unseen_or_remembered_tiles_names_nothing_on_them() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        let there = at.offset(2, 0);
        let k = Place::Stratum(StratumKind::Street);
        st.ground.add(k, there, game::ItemId::new("copper_coil"), 2);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        s.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('l'))]);
        assert_eq!(s.looking.as_ref().unwrap().cursor, there);
        for v in [Visibility::Unseen, Visibility::Remembered] {
            st.player.as_mut().unwrap().set_vis(there, v);
            st.tick += 40;
            let text = draw_screen(&s, &st, 140, 40);
            assert!(
                !text.contains("Copper coil"),
                "{v:?}: nothing names the coil: {text}"
            );
        }
        // In sight, the box names it.
        st.player
            .as_mut()
            .unwrap()
            .set_vis(there, Visibility::Visible);
        st.tick += 40;
        assert!(draw_screen(&s, &st, 140, 40).contains("Copper coil"));
    }

    #[test]
    fn the_look_cursor_stays_in_view() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        draw_screen(&s, &st, 120, 40);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('L'))]);
        }
        let map = s.last_map.get();
        let origin = s.cam.origin(map.width, map.height);
        let c = s.looking.as_ref().unwrap().cursor;
        assert!(
            tile_to_screen(map, origin, c).is_some(),
            "the cursor stays on screen: {c:?}"
        );
    }

    #[test]
    fn look_from_a_window_starts_on_its_subject() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (at, jack) = {
            let city = st.city.as_ref().unwrap();
            city.connectors
                .iter()
                .filter(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
                .find_map(|k| {
                    k.pos
                        .neighbors4()
                        .find(|n| {
                            city.walkable(StratumKind::Street, *n)
                                && city.connectors_at(*n).next().is_none()
                        })
                        .map(|n| (n, k.pos))
                })
                .unwrap()
        };
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        let ScreenCmd::Push { to: mut w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the terminal")
        };
        assert!(matches!(
            w.update(&mut st, &[key(KeyCode::Char('x'))]),
            ScreenCmd::Pop { .. }
        ));
        s.update(&mut st, &[]);
        assert_eq!(s.looking.as_ref().map(|l| l.cursor), Some(jack));
        st.tick += 40;
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains("Access terminal") && text.contains("space · interact"),
            "{text}"
        );
        // Space on it opens its window again; looking ends.
        let ScreenCmd::Push { to: w, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!("space in look opens the subject's window")
        };
        assert!(s.looking.is_none());
        st.tick += 40;
        assert!(draw_screen(&*w, &st, 120, 40).contains("i  Interact: Terminal"));
    }

    #[test]
    fn space_in_look_on_nothing_in_reach_does_nothing() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        s.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('l'))]);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
        assert!(s.looking.is_some());
    }

    #[test]
    fn tab_walks_the_points_of_interest_nearest_first() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        // On a bare tile, so the cursor does not start on something worth a look.
        let at = bare_sidewalk(&st);
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        draw_screen(&s, &st, 120, 40);
        // Let the camera settle on the player before reading the view.
        for _ in 0..60 {
            st.tick += 1;
            s.update(&mut st, &[]);
        }
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        let (place, from) = player(&st);
        let map = s.last_map.get();
        let origin = s.cam.origin(map.width, map.height);
        let view = TileRect::new(origin.x, origin.y, map.width as i32, map.height as i32);
        let pois = {
            let city = st.city.as_ref().unwrap();
            let m = s.map(&st, city);
            crate::look::points_of_interest(&st, &m, place, view, from, &|_, _| None)
        };
        assert!(pois.len() >= 2, "{pois:?}");
        assert!(
            pois.windows(2)
                .all(|w| w[0].chebyshev(from) <= w[1].chebyshev(from))
        );
        s.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(s.looking.as_ref().unwrap().cursor, pois[0]);
        s.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(s.looking.as_ref().unwrap().cursor, pois[1]);
        s.update(&mut st, &[key(KeyCode::BackTab)]);
        assert_eq!(s.looking.as_ref().unwrap().cursor, pois[0]);
    }

    #[test]
    fn look_is_free_on_the_ice_turn() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        engaged_at_the_gate(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(!crate::net::your_turn(&st), "the ICE acts");
        let before = last(&st).map(String::from);
        s.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert!(s.looking.is_some());
        assert_ne!(last(&st), Some("not your turn"), "was {before:?}");
    }

    #[test]
    fn enter_no_longer_jacks_in() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (_, _) = by_the_terminal_with_stacks(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn j_asks_before_jacking_out_and_n_stays() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        by_the_terminal_with_stacks(&mut st, &mut s);
        crate::net::jack_in_here(&mut st).unwrap();
        s.update(&mut st, &[]);
        assert_eq!(s.place, Place::Stratum(StratumKind::Net));
        let ScreenCmd::Push { to: mut ask, .. } = s.update(&mut st, &[key(KeyCode::Char('J'))])
        else {
            panic!("J asks")
        };
        st.tick += 40;
        let text = draw_screen(&*ask, &st, 120, 40);
        assert!(
            text.contains("Jack out?")
                && text.contains("y  Jack out")
                && text.contains("n  Stay in"),
            "{text}"
        );
        assert!(matches!(
            ask.update(&mut st, &[key(KeyCode::Char('n'))]),
            ScreenCmd::Pop { .. }
        ));
        assert!(st.player.as_ref().unwrap().jacked_in());
        let ScreenCmd::Push { to: mut ask, .. } = s.update(&mut st, &[key(KeyCode::Char('J'))])
        else {
            panic!("J asks again")
        };
        ask.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert_eq!(last(&st), Some("jacked out"));
    }

    #[test]
    fn j_when_not_jacked_in_says_so() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('J'))]),
            ScreenCmd::Stay
        ));
        assert_eq!(last(&st), Some("not jacked in"));
    }

    #[test]
    fn j_jacks_out_mid_fight_with_a_run_armed() {
        use crate::net::Phase;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        in_construct_at(&mut st, &mut s, 255);
        lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(crate::net::phase(&st), Some(Phase::Act));
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(st.net.run.is_some(), "a run is armed: {:?}", last(&st));
        let ScreenCmd::Push { to: mut ask, .. } = s.update(&mut st, &[key(KeyCode::Char('J'))])
        else {
            panic!("J asks mid-fight: {:?}", last(&st))
        };
        ask.update(&mut st, &[key(KeyCode::Char('J'))]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn j_on_the_ice_turn_says_not_your_turn() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        engaged_at_the_gate(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(!crate::net::your_turn(&st), "the ICE acts");
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('J'))]),
            ScreenCmd::Stay
        ));
        assert_eq!(last(&st), Some("not your turn"));
        assert!(st.player.as_ref().unwrap().jacked_in());
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
        assert_eq!(e.place, Place::Construct(b));
        assert_eq!(e.round, 1);
        assert_eq!(e.order.len(), 3, "{:?}", e.order);
        assert_eq!(e.order[0], Actor::You, "ties go to you: {:?}", e.order);
        assert_eq!(&e.order[1..], &[Actor::Ice(0), Actor::Ice(1)]);
        assert_eq!((e.moves, e.ap), (BASE_MOVE, BASE_AP));
        assert!(!e.manual);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("TURN 1  YOU › grey ICE › grey ICE"), "{text}");
        assert!(text.contains("MOV ▰▰▰▰ 4/4    AP ●● 2/2"), "{text}");
        assert!(logged(&st, "ENGAGED · the ICE is on you"));
    }

    #[test]
    fn initiative_puts_black_ice_before_grey_and_you_before_a_tie() {
        use crate::net::{Actor, initiative};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 255);
        // The deck adds one Net: at a base of 0 you score 1, under grey's 2 and black's 3.
        st.character.attributes.net = 0;
        let order = initiative(&st, Place::Construct(b));
        assert_eq!(
            order,
            vec![Actor::Ice(1), Actor::Ice(0), Actor::You],
            "black 3, grey 2, you 1"
        );
        // At 3 you tie black and the tie is yours.
        st.character.attributes.net = 2;
        assert_eq!(
            initiative(&st, Place::Construct(b)),
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
        assert_eq!(player(&st).1, here, "moves done: the key moves the cursor");
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
        // On the grid with no ICE near there is nothing to engage.
        let mut st = state8();
        st.grid_patrols = crate::grid::Patrols::Sweeps;
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
        jack_out(&mut st, &mut s);
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
        // In a fight the footer reads the run; no tether.
        assert!(crate::net::tether_view(&st, s.place).is_none());
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
    fn on_a_sites_net_a_door_node_holds_a_step_and_a_walk_and_a_hub_rides_the_riser() {
        use crate::sitenet::testutil::{across_a_door_node, hack, office_copy};
        let (mut st, b) = office_copy(190);
        let (locked, _, door) = across_a_door_node(&mut st, b);
        let place = player(&st).0;
        let mut s = MapScreen::walking(&mut st);
        let d = Dir::CARDINAL
            .into_iter()
            .find(|d| locked.step(*d) == door.at)
            .unwrap();
        crate::constructs::land(&mut st, place, locked);
        assert!(matches!(s.doorway(&mut st, d), Some(ScreenCmd::Stay)));
        assert!(last(&st).is_some_and(|l| l.contains(&door.name)));
        // A walk over the door stops before it.
        s.walking = Some(Walk {
            path: vec![door.at],
            next_at: st.clock_tick(),
        });
        s.walk_step(&mut st);
        assert_eq!(player(&st).1, locked);
        assert!(s.walking.is_none());
        hack(
            &mut st,
            door.node,
            citygen::NodeKind::DoorLock,
            game::Op::Crack,
        );
        assert!(s.doorway(&mut st, d).is_none(), "the step goes on");
        // On the hub `>` rides the riser up.
        let hub = st.constructs.site(b).unwrap().net_level(0).unwrap().hub;
        let g = Place::SiteNet {
            building: b,
            level: 0,
        };
        crate::constructs::land(&mut st, g, hub);
        assert!(matches!(s.vertical(&mut st, true), Some(ScreenCmd::Stay)));
        assert!(matches!(player(&st).0, Place::SiteNet { level, .. } if level > 0));
        assert!(last(&st).is_some_and(|l| l.starts_with("Entering")));
    }

    #[test]
    fn a_sites_gate_lets_you_through_and_out_once_its_firewall_is_broken() {
        use crate::sitenet::testutil::{at_the_gate, office_copy};
        let (mut st, b) = office_copy(190);
        at_the_gate(&mut st, b);
        let mut s = MapScreen::walking(&mut st);
        let gate = crate::constructs::city_gate(st.city.as_ref().unwrap(), b).unwrap();
        let toward = |from: Pos, to: Pos| Dir::ALL.into_iter().find(|d| from.step(*d) == to);
        let onto_gate = toward(player(&st).1, gate).unwrap();
        // A lock whole: the gate refuses you on the grid.
        assert!(matches!(
            s.gateway(&mut st, onto_gate),
            Some(ScreenCmd::Stay)
        ));
        assert_eq!(last(&st), Some("FIREWALL · 3 locks · 0/3 broken"));
        assert_eq!(player(&st).0, Place::Stratum(StratumKind::Net));
        let now = st.net.now(&st);
        for i in 0..3 {
            st.net.break_lock(b, i, now);
        }
        s.gateway(&mut st, onto_gate);
        let g = Place::SiteNet {
            building: b,
            level: 0,
        };
        assert_eq!(player(&st).0, g);
        // Inside, beside the gate: a lock closed again holds you on it.
        let entry = st.constructs.site(b).unwrap().entry;
        let beside = {
            let k = st.constructs.site(b).unwrap();
            (entry.neighbors4())
                .find(|p| {
                    k.tiles.get(*p).is_some_and(|t| t.is_walkable())
                        && k.level_at(*p).is_some_and(|l| l.level == 0)
                })
                .unwrap()
        };
        let out = toward(beside, entry).unwrap();
        st.net.firewalls.clear();
        st.net.break_lock(b, 0, now);
        crate::constructs::land(&mut st, g, beside);
        assert!(matches!(s.gateway(&mut st, out), Some(ScreenCmd::Stay)));
        assert_eq!(player(&st), (g, entry), "on the gate, held");
        assert_eq!(last(&st), Some("FIREWALL · 3 locks · 1/3 broken"));
        // Every lock broken: the step onto it goes out onto the city's gate tile.
        for i in 1..3 {
            st.net.break_lock(b, i, now);
        }
        crate::constructs::land(&mut st, g, beside);
        s.gateway(&mut st, out);
        assert_eq!(player(&st), (Place::Stratum(StratumKind::Net), gate));
        assert_eq!(last(&st), Some("Entering the GRID"));
        // A walk onto the gate inside goes out too.
        s.gateway(&mut st, onto_gate);
        crate::constructs::land(&mut st, g, beside);
        s.walking = Some(Walk {
            path: vec![entry],
            next_at: st.clock_tick(),
        });
        s.walk_step(&mut st);
        assert_eq!(player(&st), (Place::Stratum(StratumKind::Net), gate));
        assert!(s.walking.is_none());
    }

    #[test]
    fn on_a_sites_gate_inside_a_step_outward_goes_out_once_its_firewall_is_broken() {
        use crate::sitenet::testutil::{office_copy, on_hub, walk_to_the_gate_inside};
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        walk_to_the_gate_inside(&mut st, b);
        let mut s = MapScreen::walking(&mut st);
        let g = Place::SiteNet {
            building: b,
            level: 0,
        };
        let (entry, outward) = {
            let k = st.constructs.site(b).unwrap();
            let out = (Dir::CARDINAL.into_iter())
                .find(|d| k.level_at(k.entry.step(*d)).is_none())
                .unwrap();
            (k.entry, out)
        };
        let gate = crate::constructs::city_gate(st.city.as_ref().unwrap(), b).unwrap();
        // A lock whole: the gate refuses the step outward, and says why.
        assert!(matches!(s.gateway(&mut st, outward), Some(ScreenCmd::Stay)));
        assert_eq!(last(&st), Some("FIREWALL · 3 locks · 0/3 broken"));
        assert_eq!(player(&st), (g, entry));
        // Every lock broken: the step outward goes out onto the city's gate tile.
        let now = st.net.now(&st);
        for i in 0..3 {
            st.net.break_lock(b, i, now);
        }
        s.gateway(&mut st, outward);
        assert_eq!(player(&st), (Place::Stratum(StratumKind::Net), gate));
        assert_eq!(last(&st), Some("Entering the GRID"));
        // A walk from the gate inside, outward, goes out too.
        crate::constructs::enter_construct(&mut st, b).unwrap();
        walk_to_the_gate_inside(&mut st, b);
        s.walking = Some(Walk {
            path: vec![entry.step(outward)],
            next_at: st.clock_tick(),
        });
        s.walk_step(&mut st);
        assert_eq!(player(&st), (Place::Stratum(StratumKind::Net), gate));
        assert!(s.walking.is_none());
    }

    #[test]
    fn beside_a_sites_kiosk_space_and_j_jack_you_in_on_its_node() {
        use crate::sitenet::testutil::{office_copy, stand_beside_the_kiosk};
        let (mut st, b) = office_copy(190);
        let kiosk = stand_beside_the_kiosk(&mut st, b);
        let body = player(&st);
        let mut s = MapScreen::walking(&mut st);
        let cmd = jack_in(&mut st, &mut s);
        assert!(
            matches!(cmd, ScreenCmd::Pop { .. }),
            "the window closes: {:?}",
            last(&st)
        );
        let p = st.player.as_ref().unwrap();
        let k = st.constructs.site(b).unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        assert_eq!(k.device_at(p.pos).map(|d| d.node), Some(kiosk));
        assert_eq!(p.body, Some(body));
        assert_eq!(s.place, p.place, "the map follows you in");
    }

    #[test]
    fn a_sites_floor_draws_its_kiosk() {
        use crate::render::netlook::TERMINAL;
        use crate::sitenet::testutil::{office_copy, stand_beside_the_kiosk};
        let (mut st, b) = office_copy(190);
        let kiosk = stand_beside_the_kiosk(&mut st, b);
        let s = MapScreen::walking(&mut st);
        let buf = draw_buffer(&s, &st, 120, 40);
        let area = s.last_map.get();
        let origin = s.cam.origin(area.width, area.height);
        let (x, y) = crate::render::viewport::tile_to_screen(area, origin, kiosk.at)
            .expect("the kiosk in view");
        let cell = &buf[(x, y)];
        assert_eq!(
            (cell.symbol(), cell.fg),
            ("◇", TERMINAL.to_color()),
            "the kiosk's wall tile"
        );
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
        // Moves done: a direction moves the fire cursor now, not you.
        let before = player(&st).1;
        s.update(&mut st, &[key(back)]);
        assert_eq!(player(&st).1, before);
        assert!(s.target.is_some(), "the cursor is up on the act half");
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
        // The attack is a shot now: no bolt, a white streak from you to the piece.
        assert!(bolts(&st).is_empty());
        let streaks: Vec<(Pos, Pos)> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, game::Fx::Streak { colour } if colour == (0, 229, 255)))
            .map(|i| (i.origin, i.targets[0]))
            .collect();
        assert_eq!(streaks, vec![(me, gate)]);
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
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
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
        st.net.minute(
            now + game::HACK_MINUTES,
            Some(b),
            &mut st.constructs,
            &st.backdoors.roots(),
        );
        st.net.sync_guards(
            &mut st.constructs,
            b,
            now + game::HACK_MINUTES,
            &st.backdoors.roots(),
        );
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
        assert!(text.contains("AIM"), "{text}");
        assert!(text.contains("[1] CRACK ●"), "{text}");
        // A direction moves the fire cursor now; the aim's key still says so.
        let here = player(&st).1;
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(player(&st).1, here);
        s.update(&mut st, &[key(KeyCode::Char('g'))]);
        assert_eq!(last(&st), Some("moves are done"));
        assert!(text.contains("1-5 program"), "{text}");
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
        assert_eq!(
            st.fx
                .iter()
                .filter(|i| matches!(i.fx, game::Fx::Streak { .. }))
                .count(),
            1,
            "a white streak to the piece"
        );
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
        assert!(logged(&st, "Entering the GRID"), "{:?}", last(&st));
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
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
        let down = |st: &AppState| {
            let k = st.constructs.get(b).unwrap();
            k.guard_at(k.guards[0].pos).is_none()
        };
        assert!(down(&st), "broken");
        // A second alarm does not stand it; an alert ending early does not either.
        crate::net::raise_alarm(&mut st, b);
        assert!(down(&st), "the alarm leaves a broken piece broken");
        st.net.trace(b).alert_until = Some(now + 10);
        st.net
            .minute(now + 11, Some(b), &mut st.constructs, &st.backdoors.roots());
        assert!(down(&st), "the alert's end leaves it broken");
        // The hour brings it back.
        st.net.minute(
            now + game::HACK_MINUTES,
            Some(b),
            &mut st.constructs,
            &st.backdoors.roots(),
        );
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

    #[test]
    fn an_alarm_with_every_piece_broken_leaves_the_beat_running() {
        use crate::net::BEAT_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (vault, vault_pos) = node_of(&st, b, NodeKind::DataVault);
        let here = beside(&st, b, vault_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        // Both pieces broken.
        let now = st.net.now(&st);
        for i in 0..2 {
            let target = game::HackTarget::Guard {
                building: b,
                index: i,
            };
            let kind = crate::net::target_kind(&st, b, target).unwrap();
            st.net.land(b, target, kind, game::Op::Attack, now);
        }
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
        st.net.force_roll = Some(10);
        crate::net::start_run(&mut st, ItemId::new("copy"), game::HackTarget::Node(vault)).unwrap();
        st.net.trace(b).heat = game::ALARM_HEAT - 1;
        st.net.force_roll = Some(1);
        st.net.beat = st.clock_tick();
        st.tick += BEAT_TICKS;
        s.update(&mut st, &[]);
        assert!(logged(&st, "ALARM"), "{:?}", last(&st));
        assert!(
            st.net.engaged.is_none(),
            "nobody stands to fight: the board stays calm"
        );
        // The copy rolls on with the beat, no key needed.
        st.net.force_roll = Some(10);
        st.tick += BEAT_TICKS;
        s.update(&mut st, &[]);
        assert_eq!(last(&st), Some("copy 1/4"), "{:?}", last(&st));
    }

    #[test]
    fn breaking_the_last_piece_clears_the_alarm_but_escaping_does_not() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        // The vault guard already down; the gate guard beside you; the alarm on.
        let now = st.net.now(&st);
        let far = game::HackTarget::Guard {
            building: b,
            index: 1,
        };
        let kind = crate::net::target_kind(&st, b, far).unwrap();
        st.net.land(b, far, kind, game::Op::Attack, now);
        st.net
            .sync_guards(&mut st.constructs, b, now, &st.backdoors.roots());
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        let near = beside(&st, b, gate);
        stand(&mut st, &mut s, Place::Construct(b), near);
        let now = st.net.now(&st);
        st.net.trace(b).alert_until = Some(now + game::ALERT_MINUTES);
        crate::net::raise_alarm(&mut st, b);
        assert!(st.net.traces[&b].alert_until.is_some());
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
        for _ in 0..6 {
            if logged(&st, "grey ICE broken") {
                break;
            }
            roll(&mut st, &mut s);
        }
        assert!(logged(&st, "grey ICE broken"), "{:?}", last(&st));
        assert!(logged(&st, "ALARM cleared"), "{:?}", last(&st));
        assert_eq!(st.net.traces[&b].alert_until, None, "the alert is over");
        assert!(
            st.net.traces[&b].heat < game::ALARM_HEAT,
            "heat {}",
            st.net.traces[&b].heat
        );
        // Escaping clears nothing: a fresh alarm outlives a jack-out.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = engaged_at_the_gate(&mut st, &mut s);
        let now = st.net.now(&st);
        st.net.trace(b).alert_until = Some(now + game::ALERT_MINUTES);
        crate::net::raise_alarm(&mut st, b);
        let until = st.net.traces[&b].alert_until;
        assert!(until.is_some());
        jack_out(&mut st, &mut s);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        s.update(&mut st, &[]);
        assert_eq!(st.net.traces[&b].alert_until, until, "still on alert");
    }

    /// The player in the flat beside the rig, the screen walking.
    fn home_screen() -> (AppState, MapScreen) {
        let mut st = state_home();
        let mut s = MapScreen::walking(&mut st);
        let h = st.home.unwrap();
        let flat = Place::Floor {
            building: h.home.building,
            level: h.home.level,
        };
        let at = beside_rig(&st);
        stand(&mut st, &mut s, flat, at);
        (st, s)
    }

    /// A building with a gate and a server, not one of `not`.
    fn other_construct(st: &mut AppState, not: &[citygen::BuildingId]) -> citygen::BuildingId {
        let city = st.city.clone().unwrap();
        // A construct of today's kind: a site's gate is its firewall's.
        let candidates: Vec<citygen::BuildingId> = (city.buildings.iter())
            .map(|b| b.id)
            .filter(|b| !not.contains(b) && !crate::sitenet::is_site(st, *b))
            .collect();
        candidates
            .into_iter()
            .find(|b| {
                crate::constructs::city_gate(&city, *b).is_some()
                    && st.constructs.ensure(&city, *b).is_some_and(|k| {
                        k.nodes
                            .iter()
                            .any(|(id, _)| city.net.node(*id).kind == NodeKind::Server)
                    })
            })
            .expect("a building with a construct and a server")
    }

    /// The world clock moves on by `n` game minutes.
    fn advance_minutes(st: &mut AppState, n: u64) {
        let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) else {
            panic!("no city, no clock")
        };
        world.advance(city, world.tick() + n * sim::TICKS_PER_MINUTE);
    }

    fn log_text(st: &AppState) -> String {
        st.log
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_trace_burns_relays_on_the_minute_and_the_raid_takes_the_rig() {
        let (mut st, _screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&mut st, &[home]);
        st.backdoors.plant_net(a, 1, seed, 0);
        // one you keep, to lose in the raid
        let kept = other_construct(&mut st, &[home, a]);
        st.backdoors.plant_net(kept, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        crate::net::dump(&mut st, "the trace found you");
        advance_minutes(&mut st, crate::backdoors::HOLD_NET);
        crate::net::tick_backdoors(&mut st);
        assert!(log_text(&st).contains("trace passes"), "{}", log_text(&st));
        assert!(st.backdoors.held(a).is_none());
        assert!(st.backdoors.held(kept).is_some());
        advance_minutes(&mut st, crate::backdoors::LAST_LEG);
        crate::net::tick_backdoors(&mut st);
        assert!(log_text(&st).contains("they kicked your door in"));
        assert!(st.backdoors.held(kept).is_none());
        assert!(st.backdoors.seized(st.net.now(&st)));
        assert_eq!(
            crate::net::jack_in_refusal(&st),
            Some("the rig is gone · evidence".to_string())
        );
        let mails = st.journal.mails.len();
        advance_minutes(&mut st, crate::backdoors::SEIZED_MINUTES);
        crate::net::tick_backdoors(&mut st);
        assert_eq!(st.journal.mails.len(), mails + 1);
        let m = st.journal.mails.last().unwrap();
        assert_eq!(m.subject, "lose this number");
        assert!(!st.backdoors.seized(st.net.now(&st)));
    }

    #[test]
    fn the_raid_while_jacked_in_ends_the_run_cleanly() {
        let (mut st, _screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&mut st, &[home]);
        st.backdoors.plant_net(a, 1, seed, 0);
        // a trace already on its last leg, then jack in from the rig
        st.backdoors.start_route(None);
        let now = st.net.now(&st);
        st.backdoors.start_trace(now);
        st.backdoors.end_route();
        crate::net::land_at(&mut st, a).unwrap();
        advance_minutes(&mut st, crate::backdoors::LAST_LEG);
        crate::net::tick_backdoors(&mut st);
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in(), "dumped home");
        assert!(st.net.run.is_none() && st.net.engaged.is_none());
        assert!(st.backdoors.trace.is_none(), "no second trace");
        assert!(log_text(&st).contains("the line went dead"));
    }

    #[test]
    fn the_route_row_sits_under_the_rule_and_only_when_it_matters() {
        let (mut st, screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&mut st, &[home]);
        st.backdoors.plant_net(a, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        let name = crate::helix::hosts::building_hostname(&st, a);
        let text = draw_screen(&screen, &st, 120, 40);
        let row = text.lines().nth(5).unwrap();
        assert!(row.contains(&format!("ROUTE  zenbo ▸ {name}")), "{text}");
        let foot: Vec<&str> = text.lines().rev().take(4).collect();
        assert!(foot.iter().all(|l| !l.contains("ROUTE")), "{text}");
        crate::net::dump(&mut st, "the trace found you");
        let text = draw_screen(&screen, &st, 120, 40);
        let row = text.lines().nth(5).unwrap();
        assert!(row.contains(&format!("TRACE ▸ {name} ·")), "{text}");
        let foot: Vec<&str> = text.lines().rev().take(4).collect();
        assert!(foot.iter().all(|l| !l.contains("TRACE ▸")), "{text}");

        // On the street with nothing running the map takes the line back.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let text = draw_screen(&s, &st, 120, 40);
        let row = text.lines().nth(5).unwrap();
        assert!(!row.contains("ROUTE") && !row.contains("TRACE"), "{text}");
        assert_eq!(layout(Rect::new(0, 0, 120, 40), false).map.height, 30);
    }

    #[test]
    fn the_route_row_takes_one_line_from_the_map() {
        let area = Rect::new(0, 0, 120, 40);
        let with = layout(area, true);
        assert_eq!((with.map.y, with.map.height), (6, 29));
        assert_eq!(
            with.route.map(|r| (r.y, r.width)),
            Some((5, with.map.width))
        );
        assert_eq!(with.side.y, 5);
        let without = layout(area, false);
        assert_eq!((without.map.y, without.map.height), (5, 30));
        assert!(without.route.is_none());
    }

    fn server_tile(st: &AppState, b: citygen::BuildingId) -> Pos {
        let city = st.city.as_ref().unwrap();
        st.constructs
            .get(b)
            .unwrap()
            .nodes
            .iter()
            .find(|(id, _)| city.net.node(*id).kind == NodeKind::Server)
            .unwrap()
            .1
    }

    /// Fires the row whose text contains `what` on a popup, returning the command.
    fn fire_row(st: &mut AppState, p: &mut dyn Screen, what: &str) -> ScreenCmd {
        st.tick += 40;
        let text = draw_screen(&*p, st, 120, 40);
        let row = text
            .lines()
            .find(|l| l.contains(what))
            .unwrap_or_else(|| panic!("no row {what}: {text}"));
        let letter = row.chars().find(|c| c.is_ascii_alphabetic()).unwrap();
        p.update(st, &[key(KeyCode::Char(letter))])
    }

    #[test]
    fn the_rig_offers_landings_and_lands_at_a_gate() {
        let (mut st, mut s) = home_screen();
        let h = st.home.unwrap();
        let seed = st.city.as_ref().unwrap().seed;
        let other = other_construct(&mut st, &[h.home.building]);
        st.backdoors.plant_net(other, 1, seed, 0);
        let ScreenCmd::Push { to: mut popup, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the popup")
        };
        // jack in opens the landing list in its place
        let ScreenCmd::Replace { to: mut list, .. } = fire_row(&mut st, &mut *popup, "Jack in")
        else {
            panic!("jack in replaces the popup with the landings")
        };
        st.tick += 40;
        let text = draw_screen(&*list, &st, 120, 40);
        assert!(
            text.contains("JACK IN") && text.contains("your block"),
            "{text}"
        );
        let row = format!("{} · root · STABLE", crate::net::building_name(&st, other));
        assert!(text.contains(&row), "{text}");
        // land through the list
        let cmd = fire_row(&mut st, &mut *list, &row);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        assert!(
            last(&st).unwrap().contains("via your backdoor"),
            "{:?}",
            last(&st)
        );
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(
            p.pos,
            crate::constructs::city_gate(st.city.as_ref().unwrap(), other).unwrap()
        );
        assert_eq!(p.body.map(|(pl, _)| pl), Some(h.place()));
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![other]);
        assert_eq!(
            st.backdoors.held(other).unwrap().spent,
            crate::backdoors::COST_LAND_NET
        );
        // jacking out ends the route
        crate::net::jack_out(&mut st).unwrap();
        assert!(st.backdoors.route.is_none());
        // a burned one is refused and the journal shows it
        st.backdoors
            .list
            .iter_mut()
            .find(|d| d.building == other)
            .unwrap()
            .state = crate::backdoors::State::Burned {
            at: 0,
            why: crate::backdoors::Why::Decay,
        };
        assert_eq!(
            crate::net::land_at(&mut st, other),
            Err("connection refused · key rejected".to_string())
        );
        assert!(st.player.as_ref().unwrap().body.is_none());
        assert_eq!(
            st.backdoors.list[0].health(0),
            crate::backdoors::Health::Burned
        );
    }

    #[test]
    fn jacking_in_at_your_block_from_the_rig_starts_an_empty_route() {
        let (mut st, _s) = home_screen();
        crate::net::jack_in_here(&mut st).unwrap();
        assert!(st.player.as_ref().unwrap().jacked_in());
        assert_eq!(st.backdoors.route, Some(crate::backdoors::Route::default()));
        // the same at a street port starts none
        let mut st = state8();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let at = {
            let city = st.city.as_ref().unwrap();
            let jack = city
                .connectors
                .iter()
                .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
                .unwrap()
                .pos;
            jack.neighbors8()
                .find(|n| city.walkable(StratumKind::Street, *n))
                .unwrap()
        };
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), at);
        crate::net::jack_in_here(&mut st).unwrap();
        assert!(st.player.as_ref().unwrap().jacked_in());
        assert!(st.backdoors.route.is_none());
        // and a dump there starts no trace
        crate::net::dump(&mut st, "the trace found you");
        assert!(st.backdoors.trace.is_none());
    }

    #[test]
    fn a_hop_from_the_server_re_lands_at_another_gate() {
        let (mut st, _s) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&mut st, &[home]);
        let b2 = other_construct(&mut st, &[home, a]);
        st.backdoors.plant_net(a, 1, seed, 0);
        st.backdoors.plant_net(b2, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        crate::constructs::enter_construct(&mut st, a).unwrap();
        let server = server_tile(&st, a);
        st.player.as_mut().unwrap().pos = server;
        let rows = crate::interact::run_rows(
            &st,
            a,
            server,
            None,
            &mut crate::interact::FREE_LETTERS.chars(),
        );
        let hop = rows.iter().find(|c| c.label == "Hop").expect("a hop row");
        let name = crate::net::building_name(&st, b2);
        assert!(
            hop.thing.as_ref().is_some_and(|(t, _)| *t == name) && hop.enabled,
            "{:?}",
            hop.thing
        );
        assert_eq!(rows.iter().filter(|c| c.label == "Hop").count(), 1);
        crate::net::hop(&mut st, b2).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(
            p.pos,
            crate::constructs::city_gate(st.city.as_ref().unwrap(), b2).unwrap()
        );
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![a, b2]);
        // away from the server: no hop rows
        crate::constructs::enter_construct(&mut st, b2).unwrap();
        let entry = st.constructs.get(b2).unwrap().entry;
        let rows = crate::interact::run_rows(
            &st,
            b2,
            entry,
            None,
            &mut crate::interact::FREE_LETTERS.chars(),
        );
        assert!(rows.iter().all(|c| c.label != "Hop"));
        // on a street run the rows are greyed
        st.backdoors.route = None;
        let rows = crate::interact::run_rows(
            &st,
            a,
            server,
            None,
            &mut crate::interact::FREE_LETTERS.chars(),
        );
        let hop = rows.iter().find(|c| c.label == "Hop").unwrap();
        assert!(
            !hop.enabled && hop.tail.ends_with("no route home"),
            "{}",
            hop.tail
        );
    }

    #[test]
    fn a_dump_on_a_rig_run_starts_the_trace() {
        let (mut st, _s) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&mut st, &[home]);
        st.backdoors.plant_net(a, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        crate::net::dump(&mut st, "the trace found you");
        assert!(st.backdoors.route.is_none());
        let t = st.backdoors.trace.as_ref().expect("a trace");
        assert_eq!(t.relays, vec![a]);
        assert!(
            st.log
                .iter()
                .any(|e| e.text.contains("trace running · it starts at")),
        );
    }

    #[test]
    fn a_seized_rig_refuses_to_jack_in() {
        let (mut st, _s) = home_screen();
        st.backdoors.rig_seized_until = Some(u64::MAX);
        assert_eq!(
            crate::net::jack_in_refusal(&st),
            Some("the rig is gone · evidence".to_string())
        );
    }

    #[test]
    fn a_landing_that_cannot_happen_spends_nothing() {
        let (mut st, _s) = home_screen();
        let h = st.home.unwrap();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&mut st, &[h.home.building]);
        st.backdoors.plant_net(a, 1, seed, 0);
        // away from the rig
        let at = st.player.as_ref().unwrap().pos;
        st.player.as_mut().unwrap().pos = Pos::new(at.x + 20, at.y + 20);
        assert!(crate::net::land_at(&mut st, a).is_err());
        st.player.as_mut().unwrap().pos = at;
        // already jacked in
        crate::net::land_at(&mut st, a).unwrap();
        let spent = st.backdoors.held(a).unwrap().spent;
        assert_eq!(
            crate::net::land_at(&mut st, a),
            Err("already jacked in".to_string())
        );
        assert_eq!(st.backdoors.held(a).unwrap().spent, spent);
        // a street port is no rig
        crate::net::jack_out(&mut st).unwrap();
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Street);
        assert!(crate::net::land_at(&mut st, a).is_err());
        let d = st.backdoors.held(a).unwrap();
        assert_eq!(d.spent, spent);
        assert_eq!(d.state, crate::backdoors::State::Live);
    }

    #[test]
    fn a_honeypot_landing_springs_the_trap() {
        let (mut st, _s) = home_screen();
        let h = st.home.unwrap();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&mut st, &[h.home.building]);
        st.backdoors.plant_net(a, 1, seed, 0);
        st.backdoors.list[0].state = crate::backdoors::State::Honeypot;
        crate::net::land_at(&mut st, a).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.pos,
            crate::constructs::city_gate(st.city.as_ref().unwrap(), a).unwrap()
        );
        let now = st.net.now(&st);
        assert!(st.net.alert(a, now));
        assert!(st.net.trace(a).heat >= 60);
        let t = st.backdoors.trace.as_ref().expect("a trace");
        assert!(t.relays.is_empty(), "{:?}", t.relays);
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "honeypot · they were waiting · trace running")
        );
    }

    #[test]
    fn a_key_landing_costs_twenty() {
        let (mut st, _s) = home_screen();
        let h = st.home.unwrap();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&mut st, &[h.home.building]);
        st.backdoors.plant_key(a, "ana", 0, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        assert_eq!(
            st.backdoors.held(a).unwrap().spent,
            crate::backdoors::COST_LAND
        );
    }

    #[test]
    fn a_hop_aborts_a_run() {
        let (mut st, _s) = home_screen();
        let h = st.home.unwrap();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&mut st, &[h.home.building]);
        let b2 = other_construct(&mut st, &[h.home.building, a]);
        st.backdoors.plant_net(a, 1, seed, 0);
        st.backdoors.plant_net(b2, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        crate::constructs::enter_construct(&mut st, a).unwrap();
        st.net.run = Some(game::Run {
            program: ItemId::new("copper_coil"),
            target: game::HackTarget::Deck,
            done: 0,
            work: 2,
        });
        st.net.engaged = Some(crate::net::Engagement {
            place: Place::Construct(a),
            spent_at: None,
            boosted: false,
            round: 1,
            order: vec![crate::net::Actor::You],
            turn: 0,
            moves: 1,
            ap: 2,
            phase: crate::net::Phase::Act,
            manual: false,
            acting_since: 0,
        });
        crate::net::hop(&mut st, b2).unwrap();
        assert!(st.net.run.is_none());
        assert!(st.log.iter().any(|e| e.text == "run aborted"));
    }

    #[test]
    fn the_backdoor_program_is_greyed_at_the_cap() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (server, server_pos) = node_of(&st, b, NodeKind::Server);
        let here = beside(&st, b, server_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        st.character
            .unload(&st.content, &ItemId::new("crack"))
            .unwrap();
        let at = game::At {
            place: Place::Construct(b),
            pos: here,
        };
        crate::screens::console::run(&mut st, at, "program backdoor").unwrap();
        for i in 0..3 {
            st.backdoors
                .plant_net(citygen::BuildingId(9000 + i), 1, 0, 0);
        }
        let why = "the rig holds 3 lines · burn one first";
        let rows =
            |st: &AppState| crate::interact::run_rows(st, b, here, None, &mut FREE_LETTERS.chars());
        let all = rows(&st);
        let row = all.iter().find(|c| c.label == "Backdoor").expect("a row");
        assert!(!row.enabled);
        assert!(row.tail.ends_with(why), "{:?}", row.tail);
        let target = game::HackTarget::Node(server);
        assert_eq!(
            crate::net::start_run(&mut st, ItemId::new("backdoor"), target).unwrap_err(),
            why
        );
        st.backdoors
            .burn(citygen::BuildingId(9000), crate::backdoors::Why::You, 0);
        let all = rows(&st);
        assert!(all.iter().find(|c| c.label == "Backdoor").unwrap().enabled);
        assert!(crate::net::start_run(&mut st, ItemId::new("backdoor"), target).is_ok());
    }

    #[test]
    fn a_jump_puts_the_free_camera_on_the_tile_with_the_inspector_on() {
        let mut st = state();
        let city = st.city.take().unwrap();
        st.enter_city(city);
        let mut s = MapScreen::walking(&mut st);
        assert_eq!(s.mode, Mode::Walk);
        let target = Pos::new(40, 20);
        st.jump_to = Some(target);
        s.update(&mut st, &[]);
        assert_eq!(s.mode, Mode::Explore);
        assert_eq!(s.place, Place::Stratum(StratumKind::Street));
        assert_eq!(s.cursor, target);
        assert!(st.dev);
        assert!(st.sidebar.is_on(Section::Inspect));
        assert_eq!(st.jump_to, None);
    }

    /// The lot with the runner's kit on your act half, a goon at `goon`.
    fn runner_half(goon: Pos) -> (AppState, MapScreen) {
        let (mut st, mut s) = lot(goon);
        st.character = game::Character::combat_runner(&st.content);
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_some() && crate::net::your_turn(&st));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Act));
        (st, s)
    }

    fn act_ability(s: &str) -> Act {
        Act::Ability(game::AbilityId::new(s))
    }

    fn act_throw(s: &str) -> Act {
        Act::Throw(ItemId::new(s))
    }

    fn ap_left(st: &AppState) -> u8 {
        st.net.engaged.as_ref().unwrap().ap
    }

    #[test]
    fn act_list_on_the_street_and_in_a_construct() {
        let (st, _) = runner_half(Pos::new(12, 8));
        assert_eq!(
            MapScreen::act_list(&st),
            vec![
                act_ability("static"),
                act_ability("jolt"),
                act_ability("shove"),
                act_ability("redline"),
                act_ability("deadeye"),
                act_throw("frag"),
                act_throw("flashbang"),
                act_throw("emp_puck"),
                act_throw("satchel"),
                act_throw("choke_can"),
            ]
        );
        assert_eq!(MapScreen::act_key(&st, 6), Some(act_throw("frag")));
        // `0` is the tenth key, after `9`.
        assert_eq!(MapScreen::act_key(&st, 9), Some(act_throw("satchel")));
        assert_eq!(MapScreen::act_key(&st, 0), Some(act_throw("choke_can")));
        assert_eq!(MapScreen::act_key(&st, 10), None);
        let keys: String = MapScreen::act_entries(&st).iter().map(|e| e.key).collect();
        assert_eq!(keys, "1234567890");
        // A construct: the programs keep 1-5, the net's abilities follow on 6-8.
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        st.player = Some(crate::walker::Player::at(
            Place::Stratum(StratumKind::Street),
            Pos::new(6, 8),
        ));
        crate::net::start_in_net_lot(&mut st).unwrap();
        st.character = game::Character::combat_runner(&st.content);
        assert_eq!(
            MapScreen::act_list(&st),
            vec![
                act_ability("static"),
                act_ability("jolt"),
                act_ability("redline")
            ]
        );
        for n in 1..=5 {
            assert_eq!(MapScreen::act_key(&st, n), None, "{n} stays a program");
        }
        assert_eq!(MapScreen::act_key(&st, 6), Some(act_ability("static")));
        assert_eq!(MapScreen::act_key(&st, 8), Some(act_ability("redline")));
        assert_eq!(MapScreen::act_key(&st, 9), None);
        assert_eq!(MapScreen::act_key(&st, 0), None);
        // Six boosts more, nine entries in all: the keys run 6-9 and 0, then stop
        // short of the programs' 1.
        for n in 0..6 {
            let id = format!("boost_{n}");
            st.content.abilities.push(
                ron::from_str(&format!(
                    r#"Ability(id: "{id}", name: "B{n}", cooldown: 60, kind: Active(Boost(once: false)), desc: "")"#
                ))
                .unwrap(),
            );
            st.character.abilities.push(game::AbilityId::new(&id));
        }
        let keys: String = MapScreen::act_entries(&st).iter().map(|e| e.key).collect();
        assert_eq!(keys, "67890");
        assert_eq!(MapScreen::act_key(&st, 0), Some(act_ability("boost_1")));
        assert_eq!(MapScreen::act_key(&st, 1), None, "1 stays a program");
    }

    #[test]
    fn zero_throws_the_choke_can() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        let can = ItemId::new("choke_can");
        assert_eq!(st.character.count(&can), 2);
        s.update(&mut st, &[key(KeyCode::Char('0'))]);
        assert_eq!(s.armed, Some(can.clone()));
        let text = buffer_text(&draw_buffer(&s, &st, 100, 30));
        assert!(text.contains(" choke can · 6 tiles"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.character.count(&can), 1, "one thrown");
        assert_eq!(s.armed, None);
    }

    #[test]
    fn the_footer_reads_the_act_list_on_the_act_half_only() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        let text = buffer_text(&draw_buffer(&s, &st, 160, 30));
        assert!(
            text.contains(" ACT  1 static 2 jolt 3 shove 4 redline 5 deadeye 6 frag×3"),
            "{text}"
        );
        assert!(!text.contains("ABILITIES"), "{text}");
        st.character
            .fired
            .insert(game::AbilityId::new("jolt"), st.tick);
        let text = buffer_text(&draw_buffer(&s, &st, 160, 30));
        assert!(text.contains("2 jolt ◌ 3 shove"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let text = buffer_text(&draw_buffer(&s, &st, 160, 30));
        assert!(!text.contains(" ACT  "), "{text}");
    }

    #[test]
    fn a_throwable_key_arms_the_cursor_and_space_throws() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        let frag = ItemId::new("frag");
        assert_eq!(s.target, Some(Pos::new(12, 8)));
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        assert_eq!(s.armed, Some(frag.clone()));
        let text = buffer_text(&draw_buffer(&s, &st, 100, 30));
        assert!(
            text.contains(" frag · 6 tiles · blast 2 · goon 12 "),
            "{text}"
        );
        assert!(text.contains("hjkl aim · space throw · esc back"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.character.count(&frag), 2, "one thrown");
        assert_eq!(s.armed, None);
        assert_eq!(ap_left(&st), 1);
        // Esc disarms first and keeps the cursor; the next drops it.
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        assert!(s.armed.is_some());
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert_eq!((s.armed.clone(), s.target.is_some()), (None, true));
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert_eq!(s.target, None);
    }

    #[test]
    fn a_refused_throw_stays_armed_when_aiming_can_fix_it() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        s.target = Some(Pos::new(15, 8));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(s.armed.is_some(), "out of range: aim again");
        assert_eq!(last(&st), Some("out of range"));
        assert_eq!(st.character.count(&ItemId::new("frag")), 3);
        // Out of action points is no aiming problem.
        st.net.engaged.as_mut().unwrap().ap = 0;
        s.target = Some(Pos::new(12, 8));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(s.armed, None);
        assert_eq!(last(&st), Some("no AP left"));
    }

    #[test]
    fn armed_readout_warns_when_you_are_in_it() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        let me = st.player.as_ref().unwrap().pos;
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        s.target = Some(me.offset(1, 0));
        let (text, warn) = MapScreen::armed_readout(&st, &ItemId::new("frag"), me.offset(1, 0))
            .expect("a readout");
        assert!(text.ends_with(" · you're in it "), "{text:?}");
        assert!(warn);
        let (far, warn) =
            MapScreen::armed_readout(&st, &ItemId::new("frag"), Pos::new(12, 8)).unwrap();
        assert!(!far.contains("you're in it") && !warn, "{far:?}");
    }

    #[test]
    fn armed_readout_names_the_refusal() {
        let (mut st, _) = runner_half(Pos::new(12, 8));
        let frag = ItemId::new("frag");
        let (text, warn) = MapScreen::armed_readout(&st, &frag, Pos::new(15, 8)).unwrap();
        assert_eq!(text, " frag · 9 tiles · out of range ");
        assert!(!warn);
        // Out of sight there is no line.
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(10, 8), crate::render::Visibility::Unseen);
        let (text, _) = MapScreen::armed_readout(&st, &frag, Pos::new(10, 8)).unwrap();
        assert_eq!(text, " frag · 4 tiles · no line ");
        // A wall in sight cannot be landed on.
        let size = st.city.as_ref().unwrap().size;
        let wall = (0..size.h as i32)
            .flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y)))
            .filter(|p| p.chebyshev(Pos::new(6, 8)) <= 8)
            .find(|p| {
                let city = st.city.as_ref().unwrap();
                !city.walkable(StratumKind::Street, *p)
            })
            .expect("the lot has a wall in reach");
        st.player
            .as_mut()
            .unwrap()
            .set_vis(wall, crate::render::Visibility::Visible);
        let (text, _) = MapScreen::armed_readout(&st, &frag, wall).unwrap();
        assert!(text.ends_with(" · no landing "), "{text:?}");
    }

    #[test]
    fn throw_reach_grows() {
        let (mut st, _) = runner_half(Pos::new(12, 8));
        let frag = ItemId::new("frag");
        let (eleven, twelve) = (Pos::new(17, 8), Pos::new(18, 8));
        let (text, _) = MapScreen::armed_readout(&st, &frag, eleven).unwrap();
        assert_eq!(text, " frag · 11 tiles · out of range ", "bare: 8");
        // A piston arm on the left, the launcher sleeve in its forearm socket.
        st.character
            .give_item(&st.content, ItemId::new("piston_arm"), 1);
        st.character
            .give_item(&st.content, ItemId::new("launcher_sleeve"), 1);
        let copy = |st: &AppState, id: &str| {
            st.character
                .inventory
                .iter()
                .find(|s| s.item == ItemId::new(id))
                .and_then(game::Stack::copy_id)
                .unwrap()
        };
        // Free chrome: the arm goes in without a ripperdoc.
        st.free_chrome = true;
        let arm = copy(&st, "piston_arm");
        crate::actions::perform(
            &mut st,
            game::Action::InstallCopy {
                copy: arm,
                site: game::Site::LeftArm,
            },
        )
        .unwrap();
        let sleeve = copy(&st, "launcher_sleeve");
        crate::actions::perform(
            &mut st,
            game::Action::Fit {
                host: game::Host::Site(game::Site::LeftArm),
                socket: 1,
                piece: sleeve,
            },
        )
        .unwrap();
        assert_eq!(st.character.throw_bonus(&st.content), 3);
        let (text, _) = MapScreen::armed_readout(&st, &frag, eleven).unwrap();
        assert!(text.starts_with(" frag · 11 tiles · blast 2"), "{text:?}");
        assert!(!MapScreen::armed_footprint(&st, &frag, eleven).is_empty());
        let (text, _) = MapScreen::armed_readout(&st, &frag, twelve).unwrap();
        assert_eq!(text, " frag · 12 tiles · out of range ");
        // And the throw itself goes that far.
        crate::blast::throw(&mut st, &frag, eleven).unwrap();
        assert_eq!(st.character.count(&frag), 2);
    }

    #[test]
    fn the_footer_and_the_column_read_your_move() {
        let (mut st, mut s) = lot(Pos::new(14, 8));
        st.character
            .chrome
            .insert(game::Site::Spine, ItemId::new("redline_spine"));
        s.update(&mut st, &[]);
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 5);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("MOV ▰▰▰▰▰ 5/5    AP ●● 2/2"), "{text}");
        assert!(text.contains(" ◆ MOV ▰▰▰▰▰   5/5"), "{text}");
        // Their turn: your pools read whole, at your five.
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(!crate::net::your_turn(&st));
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("MOV ▰▰▰▰▰ 5/5    AP ●● 2/2"), "{text}");
        // The spine and dog legs: seven, and the column still reads them whole.
        let (mut st, mut s) = lot(Pos::new(14, 8));
        for (site, id) in [
            (game::Site::Spine, "redline_spine"),
            (game::Site::Legs, "dog_legs"),
        ] {
            st.character.chrome.insert(site, ItemId::new(id));
        }
        s.update(&mut st, &[]);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains(" ◆ MOV ▰▰▰▰▰▰▰ 7/7"), "{text}");
        assert!(text.contains("   AP  ●●      2/2"), "{text}");
    }

    #[test]
    fn a_flashbang_reads_stun_and_skips_the_chromeless_under_a_chrome_blast() {
        let (mut st, _) = runner_half(Pos::new(12, 8));
        let (text, _) =
            MapScreen::armed_readout(&st, &ItemId::new("flashbang"), Pos::new(12, 8)).unwrap();
        assert!(text.contains("goon stun"), "{text:?}");
        st.foes[0].chrome = false;
        let (text, _) =
            MapScreen::armed_readout(&st, &ItemId::new("emp_puck"), Pos::new(12, 8)).unwrap();
        assert!(!text.contains("goon"), "{text:?}");
    }

    #[test]
    fn act_keys_past_the_end_and_cooling() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        st.character
            .inventory
            .retain(|s| s.item != ItemId::new("satchel") && s.item != ItemId::new("choke_can"));
        s.update(&mut st, &[key(KeyCode::Char('9'))]);
        assert_eq!((s.armed.clone(), ap_left(&st)), (None, 2));
        s.update(&mut st, &[key(KeyCode::Char('0'))]);
        assert_eq!((s.armed.clone(), ap_left(&st)), (None, 2));
        // No frag in the pack: the entry is gone and its key does nothing.
        st.character
            .inventory
            .retain(|s| s.item != ItemId::new("frag"));
        assert_eq!(MapScreen::act_key(&st, 6), Some(act_throw("flashbang")));
        s.update(&mut st, &[key(KeyCode::Char('8'))]);
        assert_eq!((s.armed.clone(), ap_left(&st)), (None, 2));
        // A cooling jolt, the goon in reach, says so and costs nothing.
        st.foes[0].pos = Pos::new(8, 8);
        s.target = Some(Pos::new(8, 8));
        st.character
            .fired
            .insert(game::AbilityId::new("jolt"), st.tick);
        s.update(&mut st, &[key(KeyCode::Char('2'))]);
        assert!(
            last(&st).is_some_and(|l| l.contains("cooling down")),
            "{:?}",
            last(&st)
        );
        assert_eq!(ap_left(&st), 2);
    }

    #[test]
    fn boost_and_mark_keys() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        // Deadeye on the cursor's goon marks it.
        s.update(&mut st, &[key(KeyCode::Char('5'))]);
        assert!(st.foes[0].marked, "{:?}", last(&st));
        // Off a foe it asks for a target.
        let (mut st2, mut s2) = runner_half(Pos::new(12, 8));
        s2.target = Some(Pos::new(8, 8));
        s2.update(&mut st2, &[key(KeyCode::Char('5'))]);
        assert_eq!(last(&st2), Some("pick a target"));
        assert_eq!(ap_left(&st2), 2);
        // Redline fires at once and sends you back to the move half.
        s.update(&mut st, &[key(KeyCode::Char('4'))]);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Move));
    }

    #[test]
    fn the_chance_on_a_marked_foe_is_magenta_bold() {
        use crate::render::path::MARK_LIT;
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        let chance_cells = |s: &MapScreen, st: &AppState| {
            let buf = draw_buffer(s, st, 100, 30);
            let text = buffer_text(&buf);
            let row = text
                .lines()
                .position(|l| l.contains("goon · Scrap nine · 6 tiles · "))
                .unwrap_or_else(|| panic!("the readout: {text}"));
            let line: Vec<char> = text.lines().nth(row).unwrap().chars().collect();
            let pct = line.iter().position(|c| *c == '%').expect("a chance");
            let digits = (0..pct)
                .rev()
                .take_while(|x| line[*x].is_ascii_digit())
                .count();
            (
                (pct - digits..=pct)
                    .map(|x| buf[(x as u16, row as u16)].clone())
                    .collect::<Vec<_>>(),
                buf[(pct as u16 + 1, row as u16)].clone(),
            )
        };
        let (cells, _) = chance_cells(&s, &st);
        assert!(
            cells.iter().all(|c| c.fg == CYAN),
            "unmarked: the readout's own"
        );
        s.update(&mut st, &[key(KeyCode::Char('5'))]);
        assert!(st.foes[0].marked, "{:?}", last(&st));
        st.tick += 30;
        let (cells, after) = chance_cells(&s, &st);
        for c in &cells {
            assert_eq!(c.fg, MARK_LIT, "{}", c.symbol());
            assert!(c.modifier.contains(Modifier::BOLD));
        }
        assert_eq!(after.fg, CYAN, "the margin after it is not lit");
    }

    #[test]
    fn picking_a_strike_while_armed_disarms() {
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        assert!(s.armed.is_some());
        st.force_hit = Some(true);
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert_eq!(s.armed, None);
        assert_eq!(ap_left(&st), 0, "static costs two");
    }

    #[test]
    fn the_prompt_rows() {
        let (st, mut s) = runner_half(Pos::new(12, 8));
        assert_eq!(
            s.turn_prompt(&st),
            Some((
                "AIM".to_string(),
                "hjkl target · t next · space fire · 1-9 act · . end turn".to_string()
            ))
        );
        s.armed = Some(ItemId::new("frag"));
        assert_eq!(
            s.turn_prompt(&st),
            Some((
                "THROW".to_string(),
                "hjkl aim · space throw · esc back".to_string()
            ))
        );
    }

    #[test]
    fn the_pools_flash_while_a_pulse_plays() {
        let (mut st, s) = runner_half(Pos::new(12, 8));
        assert!(!s.flash_pools(&st));
        crate::casts::boost(&mut st, &game::AbilityId::new("redline")).unwrap();
        assert!(s.flash_pools(&st));
        st.tick += 100;
        assert!(!s.flash_pools(&st));
    }

    #[test]
    fn in_a_construct_the_net_abilities_follow_the_programs() {
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        st.player = Some(crate::walker::Player::at(
            Place::Stratum(StratumKind::Street),
            Pos::new(6, 8),
        ));
        crate::net::start_in_net_lot(&mut st).unwrap();
        st.character = game::Character::combat_runner(&st.content);
        let b = citygen::BuildingId(0);
        st.player.as_mut().unwrap().pos = Pos::new(37, 9);
        crate::net::engage(&mut st, Place::Construct(b), true);
        assert!(crate::net::stay(&mut st));
        st.tick = 500;
        let mut s = MapScreen::walking(&mut st);
        s.update(&mut st, &[]);
        let text = buffer_text(&draw_buffer(&s, &st, 160, 40));
        assert!(text.contains("PROGRAMS"), "{text}");
        assert!(text.contains("6 static 7 jolt 8 redline"), "{text}");
        // 6 is Static on the piece under the cursor.
        s.target = Some(st.constructs.get(b).unwrap().guards[0].pos);
        st.force_hit = Some(true);
        let before = st.net.engaged.as_ref().unwrap().ap;
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        assert!(
            st.net.engaged.as_ref().unwrap().ap < before,
            "{:?}",
            last(&st)
        );
    }

    #[test]
    fn the_scorch_lies_under_the_aim_and_the_actors_and_a_live_thing_under_a_foe() {
        use crate::render::path::{LIVE_RING_BG, RING_BG};
        use crate::ui::chrome::CYAN;
        let street = Place::Stratum(StratumKind::Street);
        let goon = Pos::new(12, 8);
        let (mut st, mut s) = runner_half(goon);
        st.force_hit = Some(true);
        // A frag at the goon, landed and bloomed out: the scorch stays under it.
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let bloom = st
            .fx
            .iter()
            .find(|i| matches!(i.fx, game::Fx::Bloom) && i.hit)
            .expect("the bloom")
            .started;
        st.tick = bloom + crate::render::fx::BLOOM_TICKS + 2;
        assert!(st.foes[0].standing());
        let buf = draw_buffer(&s, &st, 100, 30);
        // The aimed goon on the scorch: inverted on the cyan, its glyph kept.
        let c = cell_at(&s, &st, &buf, goon);
        assert_eq!((c.symbol(), c.fg, c.bg), ("@", Color::Rgb(0, 0, 0), CYAN));
        // An empty tile of the first ring: the scorch's glyph on its ground.
        let ring1 = goon.offset(0, 1);
        let c = cell_at(&s, &st, &buf, ring1);
        assert_eq!((c.symbol(), c.bg), ("░", Color::Rgb(20, 17, 16)));
        // A goon standing on a live blast: its glyph over the thing, the shading
        // under it; off it, the thing shows.
        let other = Pos::new(15, 10);
        st.foes.push(crate::foes::Foe::goon(street, other));
        st.live.push(crate::blast::Live {
            place: street,
            pos: other,
            blast: game::Blast {
                radius: 1,
                damage: 5,
                stun: 0,
                fuse: 1,
                chrome: false,
                harm: game::Harm::Kinetic,
            },
            fx: Vec::new(),
            name: "Satchel charge".to_string(),
            turns: 1,
            lands: 0,
            wakes: None,
        });
        let buf = draw_buffer(&s, &st, 100, 30);
        let c = cell_at(&s, &st, &buf, other);
        assert_eq!(
            (c.symbol(), c.fg, c.bg),
            ("@", crate::render::people::FOE, LIVE_RING_BG[0])
        );
        st.foes.pop();
        let buf = draw_buffer(&s, &st, 100, 30);
        assert_eq!(cell_at(&s, &st, &buf, other).symbol(), "o");
        st.live.clear();
        // Armed with the cursor bare on the scorch's centre: the cross on the cyan,
        // the footprint's rings over the scorch with its glyphs kept.
        st.foes[0].pos = Pos::new(18, 8);
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        assert!(s.armed.is_some() && s.target == Some(goon));
        let buf = draw_buffer(&s, &st, 100, 30);
        let c = cell_at(&s, &st, &buf, goon);
        assert_eq!((c.symbol(), c.fg, c.bg), ("×", Color::Rgb(0, 0, 0), CYAN));
        let c = cell_at(&s, &st, &buf, ring1);
        assert_eq!((c.symbol(), c.bg), ("░", RING_BG[1]));
    }

    /// Frames of one tick each on foot until `done` holds, at most `max`.
    fn frames_until(
        st: &mut AppState,
        s: &mut MapScreen,
        max: u64,
        done: impl Fn(&AppState) -> bool,
    ) -> bool {
        for _ in 0..max {
            if done(st) {
                return true;
            }
            st.tick += 1;
            s.update(st, &[]);
        }
        done(st)
    }

    #[test]
    fn a_live_blast_burns_on_the_calm_beat_on_foot() {
        use crate::net::BEAT_TICKS;
        // Calm, the goon far out of sight: nothing but the beat burns the fuse.
        let (mut st, mut s) = lot(Pos::new(30, 8));
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_none());
        let satchel = ItemId::new("satchel");
        st.character.give(satchel.clone(), 1);
        crate::blast::throw_free(&mut st, &satchel, Pos::new(10, 8)).unwrap();
        st.net.beat = st.clock_tick();
        let lands = st.live[0].lands;
        assert!(frames_until(&mut st, &mut s, 20, |st| st.tick >= lands));
        assert_eq!(st.live.len(), 1, "down, lying live");
        assert!(
            !crate::blast::live_footprints(&st, Place::Stratum(StratumKind::Street)).is_empty()
        );
        let beat = st.net.beat;
        assert!(
            frames_until(&mut st, &mut s, BEAT_TICKS + 5, |st| st.live.is_empty()),
            "the on-foot calm beat burns it"
        );
        assert_eq!(st.net.beat, st.clock_tick(), "on the beat");
        assert!(st.net.beat >= beat + BEAT_TICKS);
        assert!(
            logged(&st, "satchel charge · nothing in it"),
            "{:?}",
            last(&st)
        );
        assert!(st.net.engaged.is_none(), "still calm");
    }

    #[test]
    fn a_live_blast_wakes_the_foe_in_its_reach_when_it_lands() {
        // Nine tiles off the goon does not see you; the satchel comes down by it.
        let (mut st, mut s) = lot(Pos::new(15, 8));
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_none());
        let satchel = ItemId::new("satchel");
        st.character.give(satchel.clone(), 1);
        crate::blast::throw_free(&mut st, &satchel, Pos::new(12, 8)).unwrap();
        st.net.beat = st.clock_tick();
        // In the air: unheard.
        s.update(&mut st, &[]);
        assert_eq!(st.foes[0].mode, crate::foes::FoeMode::Idle);
        assert!(st.net.engaged.is_none());
        // Down: it hunts toward where it came from, and the street is on turns.
        let lands = st.live[0].lands;
        assert!(frames_until(&mut st, &mut s, 20, |st| st.tick > lands));
        assert_eq!(
            st.foes[0].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 8),
                lost: 0
            }
        );
        assert!(st.net.engaged.is_some(), "it hunts: turns");
        assert!(logged(&st, "ENGAGED · the goon has you"), "{:?}", last(&st));
        assert_eq!(st.live.len(), 1, "the fuse waits for your turn");
    }

    #[test]
    fn a_throw_at_your_own_feet_is_allowed_reads_red_and_hits_you_at_the_centre() {
        use crate::render::path::WARN_FG;
        let (mut st, mut s) = runner_half(Pos::new(12, 8));
        let me = st.player.as_ref().unwrap().pos;
        let frag = ItemId::new("frag");
        assert_eq!(crate::blast::landing(&st, 8, me), Ok(()));
        s.update(&mut st, &[key(KeyCode::Char('6'))]);
        s.target = Some(me);
        let (text, warn) = MapScreen::armed_readout(&st, &frag, me).unwrap();
        assert!(
            warn && text.starts_with(" frag · 0 tiles · blast 2")
                && text.ends_with(" · you're in it "),
            "{text:?}"
        );
        let buf = draw_buffer(&s, &st, 100, 30);
        let page = buffer_text(&buf);
        let row = page
            .lines()
            .position(|l| l.contains(" frag · 0 tiles"))
            .unwrap_or_else(|| panic!("the readout: {page}"));
        let col = page
            .lines()
            .nth(row)
            .unwrap()
            .chars()
            .position(|c| c == 'f')
            .unwrap();
        assert_eq!(buf[(col as u16, row as u16)].fg, WARN_FG, "drawn red");
        let hp = st.character.hp;
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.character.count(&frag), 2, "thrown");
        assert_eq!(st.character.hp, hp - 12, "the centre's full damage");
        assert!(logged(&st, "· you 12"), "{:?}", last(&st));
    }

    #[test]
    fn a_rest_stops_the_walk_under_it() {
        let mut st = crate::jobs::testutil::state_home();
        let mut s = MapScreen::walking(&mut st);
        let pos = st.player.as_ref().unwrap().pos;
        s.walking = Some(Walk {
            path: vec![pos.step(citygen::Dir::E)],
            next_at: u64::MAX,
        });
        s.aim = Some(pos);
        let until = crate::rest::now(&st) + 600;
        crate::rest::start(&mut st, crate::rest::Kind::Wait, until);
        assert!(st.rest.is_some());
        s.update(&mut st, &[]);
        assert!(s.walking.is_none() && s.aim.is_none());
    }

    /// The home state beside its bed, its map run a few frames at `w`×`h` as play does:
    /// the screen, the map rect and the map's origin.
    fn beside_the_bed(st: &mut AppState, w: u16, h: u16) -> (MapScreen, Rect, Pos) {
        use crate::missions::{Mark, marks};
        let (place, pos) = marks::resolve(st, &Mark::HomeBed, None).unwrap();
        st.player = Some(crate::walker::Player::at(place, pos));
        let mut s = MapScreen::walking(st);
        s.settle(st, (w, h));
        let map = layout(Rect::new(0, 0, w, h), false).map;
        let origin = s.cam.origin(map.width, map.height);
        (s, map, origin)
    }

    /// The Rest window as drawn on the map: from its top corner, under the title's
    /// glyph, to its bottom corner.
    fn rest_window(buf: &Buffer, map: Rect) -> Rect {
        let sym = |x: u16, y: u16| buf[(x, y)].symbol().to_string();
        let (x, y) = map
            .positions()
            .map(|p| (p.x, p.y))
            .find(|&(x, y)| {
                sym(x, y) == "┍" && x + 3 < map.right() && ["▤", "☾"].contains(&&*sym(x + 3, y))
            })
            .expect("the Rest window is drawn");
        let right = (x..map.right()).find(|&c| sym(c, y) == "┑").unwrap();
        let bottom = (y..map.bottom()).find(|&r| sym(x, r) == "┕").unwrap();
        Rect::new(x, y, right - x + 1, bottom - y + 1)
    }

    /// Your `@`'s cell and the z's cells beside it: a small one up and right, a big one
    /// two up and two right.
    fn you_and_your_zs(me: (u16, u16)) -> [(u16, u16); 3] {
        [me, (me.0 + 1, me.1 - 1), (me.0 + 2, me.1 - 2)]
    }

    fn inside(win: Rect, (x, y): (u16, u16)) -> bool {
        win.contains(ratatui::layout::Position::new(x, y))
    }

    /// Where your `@` is drawn in the map: white and bold.
    fn my_at(buf: &Buffer, map: Rect) -> (u16, u16) {
        let white = Color::Rgb(255, 255, 255);
        map.positions()
            .map(|p| (p.x, p.y))
            .find(|&(x, y)| buf[(x, y)].symbol() == "@" && buf[(x, y)].fg == white)
            .expect("your @ is on the map")
    }

    /// The summed brightness of the flat's cells on the map.
    fn flat_light(st: &AppState, buf: &Buffer, map: Rect, origin: Pos) -> u32 {
        let (_, rect) = crate::missions::marks::flat_rect(st).unwrap();
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => r as u32 + g as u32 + b as u32,
            _ => 0,
        };
        rect.iter()
            .filter_map(|p| tile_to_screen(map, origin, p))
            .map(|(x, y)| rgb(buf[(x, y)].fg) + rgb(buf[(x, y)].bg))
            .sum()
    }

    #[test]
    fn asleep_the_flat_is_dark_and_you_have_zs() {
        use crate::rest::{self, Kind};
        let mut st = crate::jobs::testutil::state_home();
        let (s, map, origin) = beside_the_bed(&mut st, 120, 40);
        let awake = draw_buffer(&s, &st, 120, 40);
        let (x, y) = my_at(&awake, map);
        assert_ne!(awake[(x + 1, y - 1)].symbol(), "z");
        let until = rest::now(&st) + 600;
        rest::start(&mut st, Kind::Sleep, until);
        assert!(st.rest.is_some());
        let asleep = draw_buffer(&s, &st, 120, 40);
        assert!(
            flat_light(&st, &asleep, map, origin) < flat_light(&st, &awake, map, origin),
            "the flat is dark"
        );
        let z = &asleep[(x + 1, y - 1)];
        assert_eq!((z.symbol(), z.fg), ("z", Color::Rgb(0, 165, 184)));
        let big = &asleep[(x + 2, y - 2)];
        assert_eq!((big.symbol(), big.fg), ("Z", CYAN));
        assert!(big.modifier.contains(Modifier::BOLD));
        assert!(!z.modifier.contains(Modifier::BOLD));
        let win = rest_window(&asleep, map);
        let title: String = (win.x..win.right())
            .map(|x| asleep[(x, win.y)].symbol())
            .collect();
        assert!(title.contains("Asleep"), "{title}");
    }

    #[test]
    fn waiting_keeps_the_lights() {
        use crate::rest::{self, Kind};
        let mut st = crate::jobs::testutil::state_home();
        let (s, map, origin) = beside_the_bed(&mut st, 120, 40);
        let awake = draw_buffer(&s, &st, 120, 40);
        let (x, y) = my_at(&awake, map);
        let until = rest::now(&st) + 60;
        rest::start(&mut st, Kind::Wait, until);
        assert!(st.rest.is_some());
        let waiting = draw_buffer(&s, &st, 120, 40);
        assert_eq!(
            flat_light(&st, &waiting, map, origin),
            flat_light(&st, &awake, map, origin)
        );
        assert_ne!(waiting[(x + 1, y - 1)].symbol(), "z");
        assert_ne!(waiting[(x + 2, y - 2)].symbol(), "Z");
    }

    #[test]
    fn asleep_fits_100x30() {
        use crate::rest::{self, Kind};
        for (w, h) in [(100, 30), (120, 40)] {
            let mut st = crate::jobs::testutil::state_home();
            let (s, map, _) = beside_the_bed(&mut st, w, h);
            let me = my_at(&draw_buffer(&s, &st, w, h), map);
            let until = rest::now(&st) + 600;
            rest::start(&mut st, Kind::Sleep, until);
            let asleep = draw_buffer(&s, &st, w, h);
            let win = rest_window(&asleep, map);
            assert!(win.x >= map.x && win.right() <= map.right() && win.bottom() <= map.bottom());
            // The flat is on the floor's bottom edge and the camera clamped there: the
            // window keeps off you and the z's above you.
            for (at, ch) in you_and_your_zs(me).into_iter().zip(["@", "z", "Z"]) {
                assert!(!inside(win, at), "{w}x{h}: {at:?} {map:?} {win:?}");
                assert_eq!(asleep[at].symbol(), ch, "{w}x{h}");
            }
        }
    }

    #[test]
    fn the_choosing_window_leaves_you_at_100x30() {
        use crate::rest::Kind;
        for (w, h) in [(100, 30), (120, 40)] {
            let mut st = crate::jobs::testutil::state_home();
            let (s, map, _) = beside_the_bed(&mut st, w, h);
            let me = my_at(&draw_buffer(&s, &st, w, h), map);
            let choose = crate::screens::rest::RestScreen::open(&st, Kind::Sleep);
            let buf = crate::screens::shots::draw_stack(&st, &[&s, &choose], (w, h));
            let win = rest_window(&buf, map);
            assert!(!inside(win, me), "{w}x{h}: {me:?} {win:?}");
            assert_eq!(buf[me].symbol(), "@", "{w}x{h}: you show");
        }
    }

    #[test]
    fn after_the_house_call_the_window_leaves_you_and_your_zs() {
        use crate::rest::{self, Kind};
        for (w, h) in [(100, 30), (120, 40)] {
            // As play has it: the house call over, beside the bed, the camera clamped to
            // the floor with the flat on its bottom edge, so your `@` is drawn low.
            let (mut st, s) = crate::screens::shots::beside_the_bed((w, h));
            let map = layout(Rect::new(0, 0, w, h), false).map;
            let me = my_at(&draw_buffer(&s, &st, w, h), map);
            assert!(
                me.1 >= map.y + map.height / 2,
                "{w}x{h}: you are low: {me:?}"
            );
            let choose = crate::screens::rest::RestScreen::open(&st, Kind::Sleep);
            let choosing = crate::screens::shots::draw_stack(&st, &[&s, &choose], (w, h));
            let win = rest_window(&choosing, map);
            assert_eq!(choosing[me].symbol(), "@", "{w}x{h}: you show choosing");
            for at in you_and_your_zs(me) {
                assert!(!inside(win, at), "{w}x{h}: choosing {at:?} {win:?}");
            }
            let until = rest::default_wake(&st, Kind::Sleep);
            rest::start(&mut st, Kind::Sleep, until);
            assert!(st.rest.is_some());
            let asleep = draw_buffer(&s, &st, w, h);
            assert_eq!(
                rest_window(&asleep, map),
                win,
                "{w}x{h}: the window stays put"
            );
            for (at, ch) in you_and_your_zs(me).into_iter().zip(["@", "z", "Z"]) {
                assert!(!inside(win, at), "{w}x{h}: asleep {at:?} {win:?}");
                assert_eq!(asleep[at].symbol(), ch, "{w}x{h}: asleep {at:?}");
            }
        }
    }

    #[test]
    fn the_consoles_wait_on_the_street_leaves_you() {
        let (w, h) = (100, 30);
        let mut st = crate::jobs::testutil::state_home();
        st.unlock_dev();
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        let mut s = MapScreen::walking(&mut st);
        s.settle(&mut st, (w, h));
        assert!(st.player.as_ref().unwrap().place.is_street());
        let map = layout(Rect::new(0, 0, w, h), false).map;
        let me = my_at(&draw_buffer(&s, &st, w, h), map);
        let typed: Vec<KeyEvent> = ":wait"
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain([key(KeyCode::Enter)])
            .collect();
        let ScreenCmd::Push { to: mut choose, .. } = s.update(&mut st, &typed) else {
            panic!("wait pushes the window");
        };
        let choosing = crate::screens::shots::draw_stack(&st, &[&s, choose.as_ref()], (w, h));
        let win = rest_window(&choosing, map);
        assert_eq!(win.bottom(), map.bottom(), "on the map's bottom rows");
        assert!(!inside(win, me), "{me:?} {win:?}");
        assert_eq!(choosing[me].symbol(), "@");
        choose.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(st.rest.is_some());
        s.update(&mut st, &[]);
        let waiting = draw_buffer(&s, &st, w, h);
        assert_eq!(rest_window(&waiting, map), win);
        assert_eq!(waiting[me].symbol(), "@");
    }

    #[test]
    fn the_consoles_wait_pushes_the_window() {
        let mut st = crate::jobs::testutil::state_home();
        st.unlock_dev();
        let mut s = MapScreen::walking(&mut st);
        let typed: Vec<KeyEvent> = ":wait"
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain([key(KeyCode::Enter)])
            .collect();
        assert!(matches!(s.update(&mut st, &typed), ScreenCmd::Push { .. }));
        assert!(s.console.is_none());
    }

    /// The lot, you on foot and nobody about: a quiet map for the mail mark.
    fn mail_lot() -> (AppState, MapScreen) {
        let (mut st, mut s) = lot(Pos::new(30, 8));
        st.foes.clear();
        s.update(&mut st, &[]);
        (st, s)
    }

    /// One map frame.
    fn map_frame(st: &mut AppState, s: &mut MapScreen) {
        st.tick += 1;
        s.update(st, &[]);
    }

    /// The text of screen row `y`.
    fn row_of(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The pill's row as drawn, when the pill is on it.
    fn drawn_pill(s: &MapScreen, st: &AppState, w: u16, h: u16) -> Option<String> {
        let buf = draw_buffer(s, st, w, h);
        let row = row_of(&buf, s.last_map.get().y + 1);
        row.contains("▣ MAIL").then_some(row)
    }

    /// Map frames while the pill is drawn: how many it was drawn on.
    fn frames_with_the_pill(st: &mut AppState, s: &mut MapScreen) -> u64 {
        let mut n = 0;
        while drawn_pill(s, st, 120, 40).is_some() {
            n += 1;
            assert!(n <= 1000, "the pill never goes");
            map_frame(st, s);
        }
        n
    }

    fn announced(st: &AppState) -> usize {
        st.log
            .iter()
            .filter(|e| e.text.contains(" · mail · "))
            .count()
    }

    #[test]
    fn a_landed_mail_logs_and_puts_the_pill_up() {
        use crate::mail_mark::{MailPill, testutil::land};
        let (mut st, mut s) = mail_lot();
        let i = land(&mut st, "Mezz", "the news");
        map_frame(&mut st, &mut s);
        let e = st.log.latest().unwrap();
        assert_eq!(
            (e.text.as_str(), e.kind),
            ("Mezz · mail · the news", Kind::Story)
        );
        assert_eq!(e.marks, vec![(0, 4)], "the sender marked");
        assert_eq!(st.mail_pill, Some(MailPill { mail: i, frames: 1 }));
        let buf = draw_buffer(&s, &st, 120, 40);
        let map = s.last_map.get();
        let y = map.y + 1;
        let row = row_of(&buf, y);
        let pill = " ▣ MAIL  MEZZ · the news ";
        let end = (map.right() - crate::mail_mark::PILL_INSET) as usize;
        let at = row.find(pill).expect(&row);
        assert_eq!(
            row[..at].chars().count() + pill.chars().count(),
            end,
            "{row:?}"
        );
        let x = (end - pill.chars().count()) as u16;
        let lit = &buf[(x + 1, y)];
        assert_eq!(
            (lit.symbol(), lit.fg, lit.bg),
            ("▣", hud::BADGE_FG, MAGENTA)
        );
        assert!(lit.modifier.contains(Modifier::BOLD));
        let soft = &buf[(x + 9, y)];
        assert_eq!(
            (soft.symbol(), soft.fg, soft.bg),
            ("M", Color::Rgb(255, 225, 245), Color::Rgb(120, 20, 90))
        );
        assert!(!row_of(&buf, 0).contains("MAIL"), "the pill is up: no mark");
        // Logged once, as it lands.
        map_frame(&mut st, &mut s);
        assert_eq!(announced(&st), 1);
    }

    #[test]
    fn the_pill_stays_six_seconds_of_map_frames_then_the_status_line_marks() {
        let (mut st, mut s) = mail_lot();
        crate::mail_mark::testutil::land(&mut st, "Mezz", "the news");
        map_frame(&mut st, &mut s);
        assert_eq!(
            frames_with_the_pill(&mut st, &mut s),
            crate::mail_mark::MAIL_PILL_TICKS
        );
        assert!(st.mail_pill.is_none());
        let buf = draw_buffer(&s, &st, 120, 40);
        let status = row_of(&buf, 0);
        assert!(status.contains(" :: ▣ NEW MAIL"), "{status:?}");
        assert!(!row_of(&buf, s.last_map.get().y + 1).contains("▣"));
    }

    #[test]
    fn a_mail_behind_the_deck_shows_when_the_map_is_back() {
        let (mut st, mut s) = mail_lot();
        let ScreenCmd::Push { to: mut deck, .. } = s.update(&mut st, &[key(KeyCode::Tab)]) else {
            panic!("tab opens the deck");
        };
        crate::mail_mark::testutil::land(&mut st, "Mezz", "the news");
        for _ in 0..600 {
            st.tick += 1;
            deck.update(&mut st, &[]);
        }
        assert_eq!(announced(&st), 0, "the deck notices nothing");
        assert!(st.mail_pill.is_none());
        // The map under the deck shows neither mark: the mail isn't noticed yet.
        let under = draw_buffer(&s, &st, 120, 40);
        assert!(!row_of(&under, 0).contains("MAIL"));
        assert!(drawn_pill(&s, &st, 120, 40).is_none());
        // The map back on top: logged, and the pill up for its full six seconds.
        map_frame(&mut st, &mut s);
        assert_eq!(announced(&st), 1);
        assert_eq!(
            frames_with_the_pill(&mut st, &mut s),
            crate::mail_mark::MAIL_PILL_TICKS
        );
    }

    #[test]
    fn the_mark_goes_when_all_is_read() {
        use crate::mail_mark::testutil::land;
        let (mut st, mut s) = mail_lot();
        let news = land(&mut st, "Mezz", "the news");
        let ten = land(&mut st, "Seren", "ten");
        map_frame(&mut st, &mut s);
        let pill = drawn_pill(&s, &st, 120, 40).expect("the pill is up");
        assert!(pill.contains(" ▣ MAIL  SEREN · ten  +1 "), "{pill:?}");
        frames_with_the_pill(&mut st, &mut s);
        let status = |st: &AppState| row_of(&draw_buffer(&s, st, 120, 40), 0);
        assert!(status(&st).contains(":: ▣ 2 NEW MAIL"), "{:?}", status(&st));
        crate::credentials::mail_read(&mut st, news);
        assert!(status(&st).contains(":: ▣ NEW MAIL"), "{:?}", status(&st));
        crate::credentials::mail_read(&mut st, ten);
        assert!(!status(&st).contains("MAIL"), "{:?}", status(&st));
    }

    #[test]
    fn the_pill_fits_a_100x30_map() {
        use crate::mail_mark::{PILL_INSET, testutil::land};
        let (mut st, mut s) = mail_lot();
        land(&mut st, "Mezz", "the news");
        map_frame(&mut st, &mut s);
        let buf = draw_buffer(&s, &st, 100, 30);
        let map = s.last_map.get();
        let y = map.y + 1;
        let row = row_of(&buf, y);
        let pill = " ▣ MAIL  MEZZ · the news ";
        let at = row.find(pill).expect(&row);
        let end = row[..at].chars().count() + pill.chars().count();
        assert_eq!(end, (map.right() - PILL_INSET) as usize, "{row:?}");
        // A subject longer than the map: cut to it, the inset kept, the rule untouched.
        land(&mut st, "Mezz", &"a long way down ".repeat(8));
        map_frame(&mut st, &mut s);
        let buf = draw_buffer(&s, &st, 100, 30);
        let lit = |x: u16| [MAGENTA, Color::Rgb(120, 20, 90)].contains(&buf[(x, y)].bg);
        let first = (map.x..map.right()).find(|x| lit(*x)).expect("the pill");
        let last = (map.x..map.right()).rev().find(|x| lit(*x)).unwrap();
        assert_eq!((first, last), (map.x, map.right() - PILL_INSET - 1));
        assert_eq!(buf[(last, y)].symbol(), "…");
        assert_eq!(buf[(map.right(), y)].symbol(), "│");
    }

    #[test]
    fn the_pill_waits_out_a_film_scene() {
        use crate::missions::{Look, scene};
        let mut st = crate::jobs::testutil::state_home();
        let mut s = MapScreen::walking(&mut st);
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        scene::play(&mut st, "house_call");
        map_frame(&mut st, &mut s);
        let film = |st: &AppState| scene::running(st).is_some_and(|r| r.look() == Look::Film);
        assert!(film(&st), "the film plays");
        crate::mail_mark::testutil::land(&mut st, "Mezz", "the news");
        for _ in 0..200 {
            map_frame(&mut st, &mut s);
            assert!(drawn_pill(&s, &st, 120, 40).is_none());
        }
        assert!(film(&st), "still playing");
        assert!(st.mail_pill.is_some_and(|p| p.frames == 0), "waiting");
        // The film over: the pill up from its first frame, for its full six seconds.
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(!film(&st));
        assert_eq!(
            frames_with_the_pill(&mut st, &mut s),
            crate::mail_mark::MAIL_PILL_TICKS
        );
    }

    /// Frames with the pill held: none drawn, its count still, the status line marking
    /// the mail instead.
    fn held_for(st: &mut AppState, s: &mut MapScreen, frames: usize) {
        for _ in 0..frames {
            map_frame(st, s);
            assert!(drawn_pill(s, st, 120, 40).is_none());
        }
        assert!(st.mail_pill.is_some_and(|p| p.frames == 0), "waiting");
        let status = row_of(&draw_buffer(s, st, 120, 40), 0);
        assert!(status.contains(" :: ▣ NEW MAIL"), "{status:?}");
    }

    #[test]
    fn a_call_up_holds_the_pill() {
        let (mut st, mut s) = mail_lot();
        let call = crate::missions::Effect::Call {
            from: "castellan".into(),
            lines: vec!["Pick up.".into()],
        };
        crate::missions::effects::apply(&mut st, usize::MAX, usize::MAX, &call);
        assert!(st.missions.call.is_some(), "the call rings");
        crate::mail_mark::testutil::land(&mut st, "Mezz", "the news");
        held_for(&mut st, &mut s, 60);
        // Sent to voicemail: the corner clear, the pill up for the voicemail, the news
        // still unread beside it.
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(st.missions.call.is_none());
        let pill = drawn_pill(&s, &st, 120, 40).expect("the pill is up");
        assert!(
            pill.contains(" ▣ MAIL  CASTELLAN · voicemail  +1 "),
            "{pill:?}"
        );
        assert_eq!(
            frames_with_the_pill(&mut st, &mut s),
            crate::mail_mark::MAIL_PILL_TICKS
        );
    }

    #[test]
    fn the_rest_window_holds_the_pill() {
        let (mut st, mut s) = mail_lot();
        let until = crate::rest::now(&st) + 600;
        crate::rest::start(&mut st, crate::rest::Kind::Wait, until);
        assert!(st.rest.is_some(), "waiting it out");
        crate::mail_mark::testutil::land(&mut st, "Mezz", "the news");
        held_for(&mut st, &mut s, 60);
        assert!(st.rest.is_some(), "still waiting");
        // Woken: the pill up for its full six seconds.
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(st.rest.is_none());
        assert_eq!(
            frames_with_the_pill(&mut st, &mut s),
            crate::mail_mark::MAIL_PILL_TICKS
        );
    }

    #[test]
    fn a_screen_on_top_does_not_eat_the_pills_frames() {
        use crate::mail_mark::MAIL_PILL_TICKS;
        let (mut st, mut s) = mail_lot();
        crate::mail_mark::testutil::land(&mut st, "Mezz", "the news");
        for _ in 0..60 {
            map_frame(&mut st, &mut s);
        }
        assert_eq!(st.mail_pill.map(|p| p.frames), Some(60));
        let ScreenCmd::Push { to: mut deck, .. } = s.update(&mut st, &[key(KeyCode::Tab)]) else {
            panic!("tab opens the deck");
        };
        for _ in 0..600 {
            st.tick += 1;
            deck.update(&mut st, &[]);
        }
        assert_eq!(
            st.mail_pill.map(|p| p.frames),
            Some(60),
            "the deck's frames"
        );
        // Back on the map: the frames it had left.
        map_frame(&mut st, &mut s);
        assert_eq!(frames_with_the_pill(&mut st, &mut s), MAIL_PILL_TICKS - 60);
    }

    #[test]
    fn mezzs_mail_lands_through_the_story_to_the_pill() {
        let mut st = crate::missions::tests::the_morning_call();
        crate::missions::tests::the_news_is_on(&mut st);
        let mut s = MapScreen::walking(&mut st);
        map_frame(&mut st, &mut s);
        // The set switched off: the book's `saw` step mails, the map's next frame tells.
        crate::missions::news::switch_off(&mut st);
        let news = st
            .journal
            .mails
            .iter()
            .position(|m| m.subject == "the news");
        assert!(news.is_some(), "Mezz's mail landed");
        let before = announced(&st);
        map_frame(&mut st, &mut s);
        assert_eq!(announced(&st), before + 1);
        assert!(
            st.log.iter().any(|e| e.text == "Mezz · mail · the news"),
            "{:?}",
            st.log.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
        assert_eq!(st.mail_pill.map(|p| p.mail), news);
        let pill = drawn_pill(&s, &st, 120, 40).expect("the pill is up");
        assert!(pill.contains(" ▣ MAIL  MEZZ · the news"), "{pill:?}");
    }

    /// The street and the Net carried on past the city's edge (#27).
    mod past_the_edge {
        use super::*;
        use crate::outside::Settlement;
        use crate::outside::stream::GroundMap;
        use crate::outside::stream::tests::beyond::{city_with_a_town, edge_out};
        use crate::outside::stream::tests::handover;
        use crate::outside::tests::home_run;
        use crate::outside::{At, TILE};
        use crate::render::map::TileMap;
        use crate::render::tileset::{MASK_E, MASK_N, MASK_S, MASK_W, connects};

        /// Seed 3's home city on foot, you on a street cell of its edge with `len`
        /// cells of open ground straight out past it; the cell and the way out.
        fn at_the_edge(len: i32) -> (AppState, MapScreen, Pos, Dir) {
            let mut st = home_run();
            st.rain = false;
            let (e, d) = edge_out(&st, len);
            let p = st.player.as_mut().unwrap();
            p.place = Place::Stratum(StratumKind::Street);
            p.pos = e;
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            (st, s, e, d)
        }

        fn you(st: &AppState) -> Pos {
            st.player.as_ref().unwrap().pos
        }

        fn off_the_map(st: &AppState, p: Pos) -> bool {
            !st.city.as_ref().unwrap().size.contains(p)
        }

        /// A call ringing, unanswered.
        fn ringing(st: &AppState) -> crate::missions::Call {
            crate::missions::Call {
                story: None,
                from: "mezz".into(),
                lines: vec!["you there?".into()],
                rang: st.clock_tick(),
                answered: None,
                line: 0,
                shown: 0,
                full: false,
            }
        }

        #[test]
        fn walking_off_the_city_stays_on_the_map_screen() {
            let (mut st, mut s, e, d) = at_the_edge(1);
            let cmd = s.update(&mut st, &[key(dir_key(d))]);
            assert!(
                matches!(cmd, ScreenCmd::Stay),
                "no screen of its own out there"
            );
            assert_eq!(
                player(&st),
                (Place::Stratum(StratumKind::Street), e.step(d))
            );
            assert!(off_the_map(&st, you(&st)));
            assert_eq!(st.outside.as_ref().unwrap().at, At::City);
            assert_eq!(s.mode, Mode::Walk);
            // and back in
            s.update(&mut st, &[key(dir_key(d.opposite()))]);
            assert_eq!(you(&st), e);
        }

        #[test]
        fn the_world_is_made_the_first_time_you_walk_out() {
            // a new game has no world out there until it's needed, as leaving needed it
            let (e, d) = edge_out(&home_run(), 1);
            let mut st = AppState::new();
            st.enter_city(crate::outside::tests::sited().clone());
            st.player = Some(Player::at(Place::Stratum(StratumKind::Street), e));
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            assert!(st.outside.is_none());
            assert!(matches!(
                s.update(&mut st, &[key(dir_key(d))]),
                ScreenCmd::Stay
            ));
            assert!(st.outside.is_some(), "the world made");
            assert_eq!(you(&st), e.step(d), "and the ground there walked onto");
        }

        #[test]
        fn the_ground_draws_past_the_edge() {
            let (mut st, mut s, _, d) = at_the_edge(4);
            clock(&mut st, "12:00");
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            assert!(off_the_map(&st, you(&st)));
            let buf = draw_buffer(&s, &st, 120, 40);
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            let o = st.outside.as_ref().unwrap();
            let at = o.origin(st.city.as_ref().unwrap());
            let ground = GroundMap {
                stream: &o.stream,
                layer: StratumKind::Street,
                minute: st.minute_of_day(),
            };
            let mask = |w: Pos, terrain: citygen::Terrain| {
                [
                    ((0, -1), MASK_N),
                    ((1, 0), MASK_E),
                    ((0, 1), MASK_S),
                    ((-1, 0), MASK_W),
                ]
                .into_iter()
                .filter(|((dx, dy), _)| {
                    ground
                        .tile(w.offset(*dx, *dy))
                        .is_some_and(|n| connects(terrain, n.terrain))
                })
                .fold(0, |m, (_, bit)| m | bit)
            };
            let mut shown = 0;
            for &p in st.player.as_ref().unwrap().beyond_sight() {
                let Some((sx, sy)) = tile_to_screen(area, origin, p) else {
                    continue;
                };
                // what floats over the map's top and bottom rows, and your `@`
                if sy <= area.y + 1 || sy + 1 >= area.bottom() || p == you(&st) {
                    continue;
                }
                let w = at.offset(p.x, p.y);
                let t = ground.tile(w).expect("what you see is built");
                let ctx = crate::render::RenderCtx {
                    style: &st.style,
                    theme: None,
                    stratum: StratumKind::Street,
                    light: ground.light(w),
                    tick: st.tick,
                    vis: crate::render::Visibility::Visible,
                    connector: None,
                    kind: None,
                    wall_mask: mask(w, t.terrain),
                    lamps_on: ground.lamps_on_at(w),
                    sky: crate::render::lighting::sky_of(&ground),
                    glow: None,
                };
                let want = crate::render::tileset::cell(t, &ctx).ch.to_string();
                assert_eq!(buf[(sx, sy)].symbol(), want, "{p:?}");
                if want != " " {
                    shown += 1;
                }
            }
            assert!(shown > 50, "{shown} cells of the ground drawn");
            // the same rain falls past the edge
            st.rain = true;
            let buf = draw_buffer(&s, &st, 120, 40);
            let drops = area
                .positions()
                .filter(|c| {
                    let p = crate::render::viewport::screen_to_tile(area, origin, (c.x, c.y));
                    off_the_map(&st, p) && buf[(c.x, c.y)].fg == Color::Rgb(90, 115, 160)
                })
                .count();
            assert!(drops > 0, "no rain past the edge");
        }

        #[test]
        fn the_camera_follows_past_the_edge() {
            let (mut st, mut s, e, d) = at_the_edge(10);
            draw_buffer(&s, &st, 120, 40);
            for _ in 0..10 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            let (dx, dy) = d.delta();
            assert_eq!(you(&st), e.offset(10 * dx, 10 * dy), "ten steps out");
            for _ in 0..30 {
                s.update(&mut st, &[]);
            }
            let buf = draw_buffer(&s, &st, 120, 40);
            let area = s.last_map.get();
            let me = (area.y..area.bottom())
                .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
                .find(|&(x, y)| {
                    buf[(x, y)].symbol() == "@" && buf[(x, y)].fg == Color::Rgb(255, 255, 255)
                })
                .expect("your @ is drawn");
            let third =
                |at: u16, from: u16, len: u16| at >= from + len / 3 && at <= from + len - len / 3;
            assert!(
                third(me.0, area.x, area.width) && third(me.1, area.y, area.height),
                "{me:?} in {area:?}"
            );
        }

        #[test]
        fn a_call_still_holds_you_at_the_edge() {
            let (mut st, mut s, e, d) = at_the_edge(1);
            st.missions.call = Some(ringing(&st));
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(you(&st), e);
            assert!(logged(&st, "not with a call on the line"));
            // in a fight
            let (mut st, mut s, e, d) = at_the_edge(1);
            crate::net::engage(&mut st, Place::Stratum(StratumKind::Street), true);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(you(&st), e);
            assert!(logged(&st, "not in a fight"));
        }

        #[test]
        fn the_net_self_walks_off_the_citys_net() {
            let (mut st, mut s, e, d) = at_the_edge(1);
            // jacked in at a port of the city's
            let city = st.city.clone().unwrap();
            let port = city
                .connectors
                .iter()
                .find(|c| {
                    c.kind == citygen::ConnectorKind::Jack
                        && c.serves.contains(&StratumKind::Street)
                        && city.walkable(StratumKind::Net, c.pos)
                })
                .expect("a street terminal")
                .pos;
            let beside = std::iter::once(port)
                .chain(port.neighbors8())
                .find(|q| city.walkable(StratumKind::Street, *q))
                .unwrap();
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), beside);
            st.player.as_mut().unwrap().jack_in(&city, None, 0).unwrap();
            let body = st.player.as_ref().unwrap().body;
            // the Net self at the Net's edge, and on
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), e);
            for _ in 0..3 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            let p = st.player.as_ref().unwrap();
            assert_eq!(p.place, Place::Stratum(StratumKind::Net));
            assert!(off_the_map(&st, p.pos), "{:?}", p.pos);
            assert_eq!(p.body, body, "the body where it was");
            // the screen draws there, and the open Net is all in sight
            draw_buffer(&s, &st, 120, 40);
            assert!(!crate::outside::away(&st), "the body is home");
        }

        #[test]
        fn landing_from_the_net_overworld_puts_the_camera_on_you() {
            let (mut st, mut s, _, _) = at_the_edge(1);
            jacked_at_a_port(&mut st, &mut s);
            s.update(&mut st, &[]);
            draw_buffer(&s, &st, 120, 40);
            let cmd = s.update(&mut st, &[key(KeyCode::Char('W'))]);
            assert!(matches!(cmd, ScreenCmd::Push { .. }), "W looks up");
            let mut up = crate::screens::outside::OutsideScreen::net_from_city(&mut st);
            let was = you(&st);
            let from = up.net_up.as_ref().unwrap().tile;
            let w = st.outside.as_ref().unwrap().world().size;
            let d = if from.x < w.w as i32 / 2 { 'l' } else { 'h' };
            let keys = vec![key(KeyCode::Char(d)); 12];
            up.update(&mut st, &keys);
            assert!(matches!(
                up.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Pop { .. }
            ));
            let landed = you(&st);
            assert!(landed.chebyshev(was) > 60, "{was:?} → {landed:?}");
            // back on the map: the camera on you at once, you seeing round you
            s.update(&mut st, &[]);
            let area = s.last_map.get();
            let (sx, sy) = on_screen(&s, &st);
            assert!(
                (sx - area.width as i32 / 2).abs() <= 1 && (sy - area.height as i32 / 2).abs() <= 1,
                "{:?} in a {}x{} view",
                (sx, sy),
                area.width,
                area.height
            );
            let buf = draw_buffer(&s, &st, 120, 40);
            let cell = &buf[(area.x + sx as u16, area.y + sy as u16)];
            assert_eq!(cell.symbol(), "@");
            let p = st.player.as_ref().unwrap();
            assert_eq!(p.pos, landed);
            assert!(
                p.vis().get(p.pos) == Some(&crate::render::Visibility::Visible)
                    || p.beyond_sight().binary_search(&p.pos).is_ok(),
                "you see where you landed"
            );
        }

        #[test]
        fn the_story_holds_still_off_the_map() {
            let (mut st, mut s, _, d) = at_the_edge(1);
            assert!(!crate::outside::away(&st));
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(off_the_map(&st, you(&st)));
            assert!(crate::outside::away(&st), "off the city's cells");
            st.missions.call = Some(ringing(&st));
            crate::clock::run(
                &mut st,
                Some(2 * crate::missions::call::RING_TICKS),
                crate::clock::PATHS_PER_FRAME,
            );
            assert_eq!(
                st.missions.call.as_ref().map(|c| c.rang),
                Some(st.clock_tick()),
                "the call rings from the start when you're back"
            );
        }

        #[test]
        fn a_held_towns_cells_draw_as_the_town() {
            // a city live whose map holds cells its town owns, the town held: those
            // cells drawn from the town's own map, not the city's
            let mut st = home_run();
            st.rain = false;
            let (c, town) = city_with_a_town(&st);
            for s in [c, town, c] {
                crate::outside::enter(&mut st, s).unwrap();
            }
            let city = st.city.as_ref().unwrap();
            let o = st.outside.as_ref().unwrap();
            let them = o.held_city(town).unwrap();
            let (live, theirs) = (o.origin(city), o.origin(them));
            let shift = Pos::new(live.x - theirs.x, live.y - theirs.y);
            let street = StratumKind::Street;
            let p = TileRect::new(0, 0, city.size.w as i32, city.size.h as i32)
                .iter()
                .find(|p| {
                    let at = live.offset(p.x, p.y);
                    let t = Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE));
                    o.owner(t) == Some(town)
                        && them
                            .tile(street, p.offset(shift.x, shift.y))
                            .map(|t| t.terrain)
                            != city.tile(street, *p).map(|t| t.terrain)
                })
                .expect("a cell the town built otherwise than the city has it");
            let mut s = MapScreen::explore(st.tick);
            s.cam.snap(p);
            s.cursor = Pos::new(-1000, -1000);
            let buf = draw_buffer(&s, &st, 120, 40);
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            let (sx, sy) = tile_to_screen(area, origin, p).unwrap();
            let pass = |map: &MapRef, at: Pos| {
                let mut out = ratatui::buffer::Buffer::empty(buf.area);
                TilePass {
                    map,
                    stratum: street,
                    origin: at,
                    tick: st.tick,
                    style: &st.style,
                    vis: &|_| crate::render::Visibility::Visible,
                    keep: None,
                }
                .draw(area, &mut out);
                out[(sx, sy)].clone()
            };
            let minute = st.minute_of_day();
            let town_drawn = pass(
                &MapRef::stratum_at(them, street, minute),
                origin.offset(shift.x, shift.y),
            );
            let city_drawn = pass(&MapRef::stratum_at(city, street, minute), origin);
            assert_eq!(buf[(sx, sy)], town_drawn, "the town's");
            assert_ne!(buf[(sx, sy)], city_drawn);
        }

        #[test]
        fn a_build_lands_while_you_are_off_the_street() {
            // asked for from the street, finished while you're in the undercity: the map
            // collects it all the same, so one that broke says so at once
            use crate::outside::stream::REACH;
            let mut st = home_run();
            let (s, b) = {
                let o = st.outside.as_ref().unwrap();
                o.others()
                    .into_iter()
                    .find(|(s, b)| s.town.is_none() && b.x > 4)
                    .unwrap()
            };
            let view = {
                let o = st.outside.as_ref().unwrap();
                let live = o.origin(st.city.as_ref().unwrap());
                let c = Pos::new(TILE * b.x - REACH, TILE * b.y + TILE * b.h / 2);
                TileRect::new(c.x - live.x, c.y - live.y, 1, 1)
            };
            crate::outside::stream::tend(&mut st, view);
            assert!(st.outside.as_ref().unwrap().stream.building(s));
            let under = StratumKind::Undercity;
            let down = st
                .city
                .as_ref()
                .unwrap()
                .stratum(under)
                .unwrap()
                .tiles
                .iter()
                .find(|(_, t)| t.is_walkable())
                .map(|(p, _)| p)
                .expect("an undercity cell to stand on");
            {
                let p = st.player.as_mut().unwrap();
                p.place = Place::Stratum(under);
                p.pos = down;
            }
            let mut map = MapScreen::walking(&mut st);
            let landed = (0..3000).any(|_| {
                map.update(&mut st, &[]);
                let o = st.outside.as_ref().unwrap();
                o.built.contains_key(&s) || {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    false
                }
            });
            assert!(landed, "collected off the street");
            assert_eq!(player(&st).0, Place::Stratum(under));
        }

        #[test]
        fn the_neighbour_draws_before_you_walk_in() {
            // on the open ground beside another city's map, never in it: once its build
            // in the background is done, its buildings draw past the edge
            use citygen::overworld::detail;
            let mut st = home_run();
            st.rain = false;
            let (s, b) = {
                let o = st.outside.as_ref().unwrap();
                o.others()
                    .into_iter()
                    .find(|(s, b)| s.town.is_none() && b.x > 4)
                    .unwrap()
            };
            let (live, beside) = {
                let o = st.outside.as_ref().unwrap();
                let w = o.world();
                // a cell of the open ground a tile west of its map you can stand on
                let beside = (b.y..b.bottom())
                    .map(|y| Pos::new(b.x - 1, y))
                    .filter(|t| o.owner(*t).is_none())
                    .find_map(|t| {
                        let at = Pos::new(TILE * t.x, TILE * t.y);
                        detail::region(w, at.x, at.y, TILE as u32, TILE as u32)
                            .iter()
                            .find(|(_, c)| c.is_walkable())
                            .map(|(p, _)| at.offset(p.x, p.y))
                    })
                    .expect("open ground beside the city");
                (o.origin(st.city.as_ref().unwrap()), beside)
            };
            let you = Pos::new(beside.x - live.x, beside.y - live.y);
            {
                let p = st.player.as_mut().unwrap();
                p.place = Place::Stratum(StratumKind::Street);
                p.pos = you;
            }
            let mut walk = MapScreen::walking(&mut st);
            draw_buffer(&walk, &st, 120, 40);
            walk.update(&mut st, &[]);
            {
                let o = st.outside.as_ref().unwrap();
                assert!(o.stream.building(s), "building as you near it");
                assert!(!o.built.contains_key(&s), "not built on the frame");
            }
            // the nearest cell of one of its buildings, on a tile it owns
            let street = StratumKind::Street;
            let (q, theirs_at) = {
                let o = st.outside.as_ref().unwrap();
                let params = crate::outside::build_params(o, s, &st.params).unwrap();
                let them = citygen::Generator::run(params).unwrap();
                let theirs = o.origin(&them);
                let w = o.world();
                them.stratum(street)
                    .unwrap()
                    .tiles
                    .iter()
                    .filter(|(q, t)| {
                        let at = theirs.offset(q.x, q.y);
                        let tile = Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE));
                        t.building.is_some()
                            && o.owner(tile) == Some(s)
                            && detail::region(w, at.x, at.y, 1, 1)[Pos::new(0, 0)].terrain
                                != t.terrain
                    })
                    .min_by_key(|(q, _)| theirs.offset(q.x, q.y).manhattan(beside))
                    .map(|(q, _)| (theirs.offset(q.x, q.y), theirs))
                    .expect("a building of theirs")
            };
            let p = Pos::new(q.x - live.x, q.y - live.y);
            let drawn = |st: &AppState| {
                let mut look = MapScreen::explore(st.tick);
                look.cam.snap(p);
                look.cursor = Pos::new(-1000, -1000);
                let buf = draw_buffer(&look, st, 120, 40);
                let area = look.last_map.get();
                let origin = look.cam.origin(area.width, area.height);
                let (sx, sy) = tile_to_screen(area, origin, p).unwrap();
                (buf[(sx, sy)].clone(), area, origin, (sx, sy))
            };
            let (before, ..) = drawn(&st);
            crate::outside::stream::finish(&mut st);
            let o = st.outside.as_ref().unwrap();
            assert_eq!(o.here(), o.home(), "never walked in");
            let them = &o.built[&s];
            assert_eq!(o.origin(them), theirs_at);
            let (after, area, origin, (sx, sy)) = drawn(&st);
            let mut out = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 120, 40));
            let shift = Pos::new(live.x - theirs_at.x, live.y - theirs_at.y);
            TilePass {
                map: &MapRef::stratum_at(them, street, st.minute_of_day()),
                stratum: street,
                origin: origin.offset(shift.x, shift.y),
                tick: st.tick,
                style: &st.style,
                vis: &|_| crate::render::Visibility::Visible,
                keep: None,
            }
            .draw(area, &mut out);
            assert_eq!(after, out[(sx, sy)].clone(), "their building");
            assert_ne!(before, after, "drawn as the ground before it was built");
        }

        /// The text of the sidebar's column, right of the map.
        fn side_text(s: &MapScreen, buf: &ratatui::buffer::Buffer) -> String {
            let x0 = s.last_map.get().right() + 1;
            let mut out = String::new();
            for y in buf.area.y..buf.area.bottom() {
                for x in x0..buf.area.right() {
                    out.push_str(buf[(x, y)].symbol());
                }
                out.push('\n');
            }
            out
        }

        #[test]
        fn here_off_the_map_names_the_place_and_the_road() {
            let (mut st, mut s, _, d) = at_the_edge(4);
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            assert!(off_the_map(&st, you(&st)));
            draw_buffer(&s, &st, 120, 40);
            let buf = draw_buffer(&s, &st, 120, 40);
            let side = side_text(&s, &buf);
            assert!(side.contains("open ground ·"), "{side}");
            let o = st.outside.as_ref().unwrap();
            let w = o.world();
            let t = crate::outside::here::tile(&st).unwrap();
            let named = match w.routes.iter().find(|r| r.path.contains(&t)) {
                Some(r) => side.contains(&r.name),
                None => {
                    w.regions.iter().any(|r| side.contains(&r.name))
                        || w.cities.iter().any(|c| side.contains(&c.name))
                }
            };
            assert!(named, "the tile's road or region: {side}");
            // and on its own street, the city's HERE
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d.opposite()))]);
            }
            let buf = draw_buffer(&s, &st, 120, 40);
            assert!(!side_text(&s, &buf).contains("open ground ·"));
        }

        #[test]
        fn a_bot_follows_you_onto_the_ground() {
            let (mut st, mut s, _, d) = at_the_edge(10);
            st.character = game::Character::debug_runner(&st.content);
            crate::bots::sync(&mut st);
            assert!(
                st.bots.first().is_some_and(|b| b.pos.is_some()),
                "a bot out"
            );
            for _ in 0..10 {
                s.update(&mut st, &[key(dir_key(d))]);
                for _ in 0..8 {
                    st.tick += 1;
                    s.update(&mut st, &[]);
                }
            }
            let me = you(&st);
            assert!(off_the_map(&st, me));
            let b = &st.bots[0];
            assert_eq!(b.place, Place::Stratum(StratumKind::Street));
            let at = b.pos.expect("the bot is out there with you");
            assert!(
                at.chebyshev(me) <= crate::bots::LEASH,
                "the bot at {at:?}, you at {me:?}"
            );
            // and drawn where it is
            let buf = draw_buffer(&s, &st, 120, 40);
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            let (sx, sy) = tile_to_screen(area, origin, at).expect("in view");
            let frames = &crate::bots::spec(&st, 0).unwrap().frames;
            assert!(
                frames
                    .iter()
                    .any(|f| buf[(sx, sy)].symbol() == f.to_string()),
                "{:?} isn't the bot",
                buf[(sx, sy)].symbol()
            );
        }

        #[test]
        fn nothing_is_dropped_on_the_open_ground() {
            let (mut st, mut s, _, d) = at_the_edge(1);
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(off_the_map(&st, you(&st)));
            let stim = ItemId::new("stim_patch");
            st.character.give_item(&st.content, stim.clone(), 2);
            let held = st.character.count(&stim);
            let drop = || game::Action::Drop {
                item: stim.clone(),
                qty: 1,
            };
            assert_eq!(
                crate::actions::perform(&mut st, drop()),
                Err("nowhere to leave it out here".to_string())
            );
            assert_eq!(st.character.count(&stim), held, "the item kept");
            let (place, pos) = player(&st);
            assert!(st.ground.at(place, pos).is_empty());
            // on the city's own street it goes down
            s.update(&mut st, &[key(dir_key(d.opposite()))]);
            assert!(crate::actions::perform(&mut st, drop()).is_ok());
            assert_eq!(st.character.count(&stim), held - 1);
        }

        #[test]
        fn the_ground_is_built_ahead_of_the_view() {
            let (mut st, mut s, _, d) = at_the_edge(10);
            draw_buffer(&s, &st, 200, 60);
            let w = st.outside.as_ref().unwrap().world().clone();
            let world = TileRect::new(0, 0, TILE * w.size.w as i32, TILE * w.size.h as i32);
            for step in 0..10 {
                s.update(&mut st, &[]);
                let area = s.last_map.get();
                let origin = s.cam.origin(area.width, area.height);
                let b = Beyond::of(&st).unwrap();
                for p in
                    TileRect::new(origin.x, origin.y, area.width as i32, area.height as i32).iter()
                {
                    if !world.contains(b.origin().offset(p.x, p.y)) {
                        continue;
                    }
                    if let Some(Owner::Ground(stream, at)) = b.elsewhere(p) {
                        assert!(
                            stream.street.tile(at).is_some(),
                            "step {step}: {p:?} not built"
                        );
                    }
                }
                s.update(&mut st, &[key(dir_key(d))]);
                draw_buffer(&s, &st, 200, 60);
            }
        }

        #[test]
        fn the_ground_underfoot_is_the_worlds_and_water_stops_you() {
            use citygen::Terrain;
            use citygen::overworld::detail;
            let (mut st, mut s, _, d) = at_the_edge(10);
            let w = st.outside.as_ref().unwrap().world().clone();
            let cell = |st: &AppState, p: Pos| {
                let c = Beyond::of(st).unwrap().origin().offset(p.x, p.y);
                detail::region(&w, c.x, c.y, 1, 1)[Pos::new(0, 0)]
            };
            for _ in 0..10 {
                s.update(&mut st, &[key(dir_key(d))]);
                let here = you(&st);
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let map = MapRef::stratum_of(city, StratumKind::Street).beyond(&b);
                let (got, want) = (*map.tile(here).unwrap(), cell(&st, here));
                assert_eq!((got.terrain, got.variant), (want.terrain, want.variant));
            }
            // somewhere on the ground beside deep water: the step in is refused
            let here = you(&st);
            crate::outside::stream::tend(
                &mut st,
                TileRect::new(here.x - 160, here.y - 160, 321, 321),
            );
            let wet = |t: &citygen::Tile| matches!(t.terrain, Terrain::Water | Terrain::Shallows);
            let (from, dir) = {
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let map = MapRef::stratum_of(city, StratumKind::Street).beyond(&b);
                TileRect::new(here.x - 150, here.y - 150, 301, 301)
                    .iter()
                    .filter(|p| !b.live_owns(*p) && map.walkable(*p))
                    .flat_map(|p| [Dir::N, Dir::E, Dir::S, Dir::W].map(|d| (p, d)))
                    .find(|(p, d)| {
                        !b.live_owns(p.step(*d)) && map.tile(p.step(*d)).is_some_and(wet)
                    })
                    .expect("water near the edge")
            };
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), from);
            s.update(&mut st, &[key(dir_key(dir))]);
            assert_eq!(you(&st), from, "too deep to wade");
        }

        #[test]
        fn an_aimed_walk_goes_over_the_ground() {
            let (mut st, mut s, _, d) = at_the_edge(6);
            for _ in 0..3 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            let from = you(&st);
            assert!(off_the_map(&st, from));
            crate::net::engage(&mut st, Place::Stratum(StratumKind::Street), true);
            s.update(&mut st, &[key(KeyCode::Char('g'))]);
            s.update(&mut st, &[key(dir_key(d)), key(dir_key(d))]);
            assert_eq!(s.aim, Some(from.step(d).step(d)), "the aim on the ground");
            s.update(&mut st, &[key(KeyCode::Enter)]);
            walk_tick(&mut st, &mut s);
            walk_tick(&mut st, &mut s);
            assert_eq!(you(&st), from.step(d).step(d));
        }

        #[test]
        fn an_aimed_walk_keeps_to_the_city_in_a_fight() {
            let (mut st, mut s, e, d) = at_the_edge(2);
            crate::net::engage(&mut st, Place::Stratum(StratumKind::Street), true);
            // the aim doesn't reach across the city's edge
            s.update(&mut st, &[key(KeyCode::Char('g'))]);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(s.aim, Some(e), "the aim stays on the city's side");
            // nor does a walk step over it
            s.aim = None;
            s.walking = Some(Walk {
                path: vec![e.step(d)],
                next_at: 0,
            });
            walk_tick(&mut st, &mut s);
            assert_eq!(you(&st), e);
            assert!(logged(&st, "not in a fight"));
        }

        #[test]
        fn reaching_the_mark_on_the_ground_clears_it() {
            let (mut st, mut s, e, d) = at_the_edge(2);
            let (dx, dy) = d.delta();
            let far = e.offset(2 * dx, 2 * dy);
            let o = st.outside.as_mut().unwrap();
            let city = st.city.as_ref().unwrap();
            let tile = o.tile_of(city, far.offset(3 * TILE * dx, 3 * TILE * dy));
            o.mark = Some(crate::outside::Mark {
                tile,
                name: "the old relay".into(),
            });
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(st.outside.as_ref().unwrap().mark.is_some(), "not there yet");
            st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
                tile: st
                    .outside
                    .as_ref()
                    .unwrap()
                    .tile_of(st.city.as_ref().unwrap(), far),
                name: "the old relay".into(),
            });
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(st.outside.as_ref().unwrap().mark.is_none());
            assert!(logged(&st, "you're at the old relay"));
        }

        #[test]
        fn frames_off_the_map_by_day_and_by_night() {
            // nothing on the screen trips over a place off the map: a track up, the
            // keys you'd press out there, the deck over it
            let (mut st, mut s, e, d) = at_the_edge(4);
            let (dx, dy) = d.delta();
            let spot = crate::track::Source::Spot {
                place: Place::Stratum(StratumKind::Street),
                pos: e.offset(-20 * dx, -20 * dy),
                radius: 0,
            };
            let _ = crate::track::set(&mut st, spot);
            for hhmm in ["12:00", "23:00"] {
                clock(&mut st, hhmm);
                for _ in 0..4 {
                    s.update(&mut st, &[key(dir_key(d))]);
                    draw_buffer(&s, &st, 120, 40);
                }
                assert!(off_the_map(&st, you(&st)));
                for k in [
                    KeyCode::Char(' '),
                    KeyCode::Esc,
                    KeyCode::Char('x'),
                    KeyCode::Char('l'),
                    KeyCode::Esc,
                    KeyCode::Char('.'),
                ] {
                    if let ScreenCmd::Push { to, .. } = s.update(&mut st, &[key(k)]) {
                        draw_buffer(to.as_ref(), &st, 120, 40);
                    }
                    draw_buffer(&s, &st, 120, 40);
                }
                if let ScreenCmd::Push { to, .. } = s.update(&mut st, &[key(KeyCode::Tab)]) {
                    draw_buffer(to.as_ref(), &st, 120, 40);
                }
                for _ in 0..4 {
                    s.update(&mut st, &[key(dir_key(d.opposite()))]);
                    draw_buffer(&s, &st, 100, 30);
                }
            }
        }

        /// A cell of the open ground beside a pump of the truck stop nearest the
        /// city, in the city's frame.
        fn by_a_pump(st: &AppState) -> Pos {
            use citygen::overworld::{PoiKind, detail};
            use citygen::{Feature, Furniture};
            let o = st.outside.as_ref().unwrap();
            let w = o.world();
            let origin = o.origin(st.city.as_ref().unwrap());
            let home = Pos::new(origin.x.div_euclid(TILE), origin.y.div_euclid(TILE));
            let poi = w
                .pois
                .iter()
                .filter(|p| {
                    p.kind == PoiKind::TruckStop
                        && (-3..=3).all(|dy| {
                            (-3..=3)
                                .all(|dx| !crate::outside::in_any_block(st, p.at.offset(dx, dy)))
                        })
                })
                .min_by_key(|p| p.at.dist2(home))
                .expect("a truck stop out on its own");
            let c = Pos::new(TILE * (poi.at.x - 3), TILE * (poi.at.y - 3));
            let g = detail::region(w, c.x, c.y, 7 * TILE as u32, 7 * TILE as u32);
            let pump = g
                .iter()
                .find(|(_, t)| t.feature == Some(Feature::Furniture(Furniture::Vending)))
                .map(|(p, _)| p)
                .expect("a pump");
            let beside = pump
                .neighbors4()
                .find(|q| g.get(*q).is_some_and(|t| t.is_walkable()))
                .expect("ground beside the pump");
            Pos::new(c.x + beside.x - origin.x, c.y + beside.y - origin.y)
        }

        /// What would stop a step from `near` out to `r` cells round it on `place`: an
        /// open cell of the ground, and the way onto one of the ground that won't
        /// take you whose tile `wanted` says yes to.
        fn refused_near(
            st: &AppState,
            place: Place,
            near: Pos,
            r: i32,
            wanted: &dyn Fn(Option<&citygen::Tile>) -> bool,
        ) -> Option<(Pos, Dir)> {
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(st).unwrap();
            let map = walked_map(st, city, place, Some(&b));
            let ground = |p: Pos| matches!(b.elsewhere(p), Some(Owner::Ground(..)));
            TileRect::new(near.x - r, near.y - r, 2 * r + 1, 2 * r + 1)
                .iter()
                .filter(|p| ground(*p) && map.walkable(*p))
                .flat_map(|p| Dir::ALL.map(|d| (p, d)))
                .find(|(p, d)| {
                    let to = p.step(*d);
                    ground(to) && !map.walkable(to) && wanted(map.tile(to))
                })
        }

        /// How many times the log says `line`.
        fn said(st: &AppState, line: &str) -> usize {
            st.log.iter().filter(|e| e.text == line).count()
        }

        #[test]
        fn a_step_the_ground_refuses_says_why_on_foot_and_nothing_on_the_net() {
            use citygen::Terrain;
            let (mut st, mut s, e, _) = at_the_edge(1);
            let street = Place::Stratum(StratumKind::Street);
            // water: too deep to wade
            crate::outside::stream::tend(&mut st, TileRect::new(e.x - 160, e.y - 160, 321, 321));
            let wet = |t: Option<&citygen::Tile>| {
                t.is_some_and(|t| matches!(t.terrain, Terrain::Water | Terrain::Shallows))
            };
            let (from, d) = refused_near(&st, street, e, 150, &wet).expect("water near the city");
            stand(&mut st, &mut s, street, from);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(you(&st), from);
            assert_eq!(last(&st), Some("too deep to wade"));
            // a pump: in the way, and nothing said, as a vending machine in the city
            let by = by_a_pump(&st);
            stand(&mut st, &mut s, street, by);
            let pump = Dir::ALL
                .into_iter()
                .find(|d| !open_at(&st, street, by.step(*d)))
                .expect("the pump beside you");
            let before = st.log.len();
            s.update(&mut st, &[key(dir_key(pump))]);
            assert_eq!(you(&st), by);
            assert_eq!(st.log.len(), before, "{:?}", last(&st));
            // on the Net out there a wall stops you as one does in the city: nothing said
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
            let net = Place::Stratum(StratumKind::Net);
            let (from, d) = refused_near(&st, net, you(&st), 40, &|t| t.is_some())
                .expect("a wall on the open Net");
            stand(&mut st, &mut s, net, from);
            let lines = [
                "too deep to wade",
                "too steep to climb",
                "the world ends here",
                "something in the way",
            ];
            let before = lines.map(|l| said(&st, l));
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(you(&st), from);
            assert_eq!(lines.map(|l| said(&st, l)), before, "the Net says nothing");
        }

        #[test]
        fn the_grounds_refusals_are_the_outside_screens_words() {
            use citygen::{Feature, Terrain, Tile};
            let line = |t: Terrain| why_not(Some(&Tile::new(t)));
            assert_eq!(why_not(None), Some("the world ends here"));
            assert_eq!(line(Terrain::Void), Some("the world ends here"));
            assert_eq!(line(Terrain::Water), Some("too deep to wade"));
            assert_eq!(line(Terrain::Shallows), Some("too deep to wade"));
            assert_eq!(line(Terrain::Cliff), Some("too steep to climb"));
            assert_eq!(line(Terrain::Rock), Some("too steep to climb"));
            // a wall, and anything standing in the way, say nothing, as in the city
            assert_eq!(line(Terrain::Wall), None);
            let mut pump = Tile::new(Terrain::Gravel);
            pump.feature = Some(Feature::Terminal);
            assert_eq!(why_not(Some(&pump)), None);
        }

        #[test]
        fn space_on_the_ground_with_no_port_in_reach_is_space_on_the_street() {
            use citygen::{Feature, Furniture};
            let (mut st, mut s, _, d) = at_the_edge(4);
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            let me = you(&st);
            assert!(off_the_map(&st, me));
            {
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let map = walked_map(&st, city, Place::Stratum(StratumKind::Street), Some(&b));
                let port = std::iter::once(me).chain(me.neighbors8()).any(|q| {
                    map.tile(q).is_some_and(|t| {
                        matches!(
                            t.feature,
                            Some(Feature::Terminal) | Some(Feature::Furniture(Furniture::Vending))
                        )
                    })
                });
                assert!(!port, "no port beside you");
            }
            let cmd = s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(
                matches!(cmd, ScreenCmd::Push { .. }),
                "space opens what's in reach, Look at least: {:?}",
                last(&st)
            );
            assert!(!logged(&st, "no port in reach"));
            assert!(!st.player.as_ref().unwrap().jacked_in());
        }

        #[test]
        fn the_top_band_off_the_map_says_where_you_are_as_here_does() {
            let (mut st, mut s, _, d) = at_the_edge(4);
            let band = |s: &MapScreen, st: &AppState| row_of(&draw_buffer(s, st, 120, 40), 0);
            assert!(band(&s, &st).contains("· Street"), "the city's own street");
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            assert!(off_the_map(&st, you(&st)));
            let place = |lines: Vec<ratatui::text::Line>| {
                lines[0]
                    .spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
                    .trim()
                    .to_string()
            };
            let here = place(crate::outside::here::here_lines(&st));
            let top = band(&s, &st);
            assert!(top.contains(&here), "{here:?} in {top:?}");
            assert!(!top.contains("Street"), "{top:?}");
            // jacked in at a pump, your Net self off the city's map: the Net HERE names
            let by = by_a_pump(&st);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), by);
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
            s.update(&mut st, &[]);
            let here = place(crate::outside::here::net_here_lines(&st));
            let top = band(&s, &st);
            assert!(top.contains(&here), "{here:?} in {top:?}");
            // and back on the city's own street, the city's band
            let (mut st, mut s, _, d) = at_the_edge(4);
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            for _ in 0..4 {
                s.update(&mut st, &[key(dir_key(d.opposite()))]);
            }
            assert!(band(&s, &st).contains("· Street"));
        }

        #[test]
        fn tab_off_the_citys_cells_opens_the_worlds_map() {
            // on the city's cells the deck's MAP is the city's; out on the open ground,
            // where your mark is off the city's map, it opens on the world, as Tab on the
            // overworld opens it
            use crate::screens::atlas::Zoom;
            let (mut st, mut s, _, d) = at_the_edge(6);
            st.atlas.zoom = Zoom::Fit;
            let cmd = s.update(&mut st, &[key(KeyCode::Tab)]);
            assert!(matches!(cmd, ScreenCmd::Push { .. }), "the deck");
            assert_eq!(st.atlas.zoom, Zoom::Fit, "the city's");
            for _ in 0..3 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            assert!(!crate::outside::stream::live_owns(&st, you(&st)));
            let cmd = s.update(&mut st, &[key(KeyCode::Tab)]);
            assert!(matches!(cmd, ScreenCmd::Push { .. }), "the deck");
            assert_eq!(st.atlas.zoom, Zoom::World, "the world's");
        }

        #[test]
        fn the_look_cursor_walks_the_ground_off_the_map() {
            let (mut st, mut s, _, d) = at_the_edge(10);
            for _ in 0..10 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            for _ in 0..30 {
                s.update(&mut st, &[]);
            }
            draw_buffer(&s, &st, 120, 40);
            let me = you(&st);
            assert!(off_the_map(&st, me));
            s.update(&mut st, &[key(KeyCode::Char('x'))]);
            // two steps further out, where nothing of the city is
            s.update(&mut st, &[key(dir_key(d)), key(dir_key(d))]);
            let (dx, dy) = d.delta();
            let cursor = s.looking.map(|l| l.cursor);
            assert_eq!(cursor, Some(me.offset(2 * dx, 2 * dy)));
            assert!(s.view_rect().contains(cursor.unwrap()));
        }

        /// Seed 3's home city on foot, you on the open ground beside another city's map
        /// (held), a step from walking into it: the city, the cell and the step.
        fn beside_the_next_city() -> (AppState, MapScreen, Settlement, Pos, Dir) {
            let mut st = home_run();
            st.rain = false;
            let (c, x, d) = handover::beside_a_city(&mut st);
            handover::on_foot(&mut st, x);
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            assert_eq!(you(&st), x, "on the ground beside it");
            (st, s, c, x, d)
        }

        /// The screen cell your `@` is drawn on, from the map's top left.
        fn on_screen(s: &MapScreen, st: &AppState) -> (i32, i32) {
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            let me = you(st);
            (me.x - origin.x, me.y - origin.y)
        }

        /// Jacked in at a street port of the live city's, your body beside it.
        fn jacked_at_a_port(st: &mut AppState, s: &mut MapScreen) {
            jacked_at_a_port_where(st, s, &|_, _| true);
        }

        /// `jacked_at_a_port`, at the first street port `ok` takes.
        fn jacked_at_a_port_where(
            st: &mut AppState,
            s: &mut MapScreen,
            ok: &dyn Fn(&citygen::City, Pos) -> bool,
        ) {
            let city = st.city.clone().unwrap();
            let port = city
                .connectors
                .iter()
                .find(|c| {
                    c.kind == citygen::ConnectorKind::Jack
                        && c.serves.contains(&StratumKind::Street)
                        && city.walkable(StratumKind::Net, c.pos)
                        && ok(&city, c.pos)
                })
                .expect("a street terminal")
                .pos;
            let beside = std::iter::once(port)
                .chain(port.neighbors8())
                .find(|q| city.walkable(StratumKind::Street, *q))
                .unwrap();
            stand(st, s, Place::Stratum(StratumKind::Street), beside);
            st.player.as_mut().unwrap().jack_in(&city, None, 0).unwrap();
        }

        /// What the live city's frame shows of the Net at `p`, past its edge too.
        fn net_at(st: &AppState, p: Pos) -> Option<citygen::Tile> {
            let city = st.city.as_ref().unwrap();
            let b = Beyond::of(st).unwrap();
            walked_map(st, city, Place::Stratum(StratumKind::Net), Some(&b))
                .tile(p)
                .copied()
        }

        #[test]
        fn walking_into_another_city_hands_over_on_the_same_cell() {
            let (mut st, mut s, c, x, d) = beside_the_next_city();
            let at = handover::world_of(&st, x.step(d));
            let cmd = s.update(&mut st, &[key(dir_key(d))]);
            assert!(matches!(cmd, ScreenCmd::Stay), "no screen of its own");
            let o = st.outside.as_ref().unwrap();
            assert_eq!(o.here(), c, "the next city is the live one");
            assert_eq!(
                handover::world_of(&st, you(&st)),
                at,
                "on the cell walked onto"
            );
            assert!(logged(&st, &format!("into {}", o.name(c))));
            assert!(crate::outside::stream::live_owns(&st, you(&st)));
            assert_eq!(s.mode, Mode::Walk);
            assert_ne!(crate::runs::describe::place_line(&st), "open ground");
            // and back out onto the ground: it stays the live city
            let back = you(&st);
            s.update(&mut st, &[key(dir_key(d.opposite()))]);
            assert_eq!(
                you(&st),
                back.step(d.opposite()),
                "back out onto the ground"
            );
            assert_eq!(st.outside.as_ref().unwrap().here(), c, "it stays live");
            assert_eq!(crate::runs::describe::place_line(&st), "open ground");
        }

        #[test]
        fn the_camera_does_not_move_on_a_handover() {
            let (mut st, mut s, c, _, d) = beside_the_next_city();
            draw_buffer(&s, &st, 120, 40);
            for _ in 0..30 {
                s.update(&mut st, &[]);
            }
            let before = on_screen(&s, &st);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(st.outside.as_ref().unwrap().here(), c);
            let (dx, dy) = d.delta();
            assert_eq!(on_screen(&s, &st), (before.0 + dx, before.1 + dy));
            // drawn there
            let buf = draw_buffer(&s, &st, 120, 40);
            let area = s.last_map.get();
            let (sx, sy) = on_screen(&s, &st);
            let cell = &buf[(area.x + sx as u16, area.y + sy as u16)];
            assert_eq!(cell.symbol(), "@");
        }

        #[test]
        fn what_you_saw_from_outside_is_remembered_inside() {
            let (mut st, mut s, c, _, d) = beside_the_next_city();
            clock(&mut st, "12:00");
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
            // the city's own cells seen from the ground, in the world's record
            let theirs: Vec<Pos> = {
                let o = st.outside.as_ref().unwrap();
                let city = o.held_city(c).unwrap();
                let at = o.origin(city);
                let map = TileRect::new(at.x, at.y, city.size.w as i32, city.size.h as i32);
                crate::outside::ground::seen_in(o, map)
                    .into_iter()
                    .filter(|w| {
                        o.owner(Pos::new(w.x.div_euclid(TILE), w.y.div_euclid(TILE))) == Some(c)
                    })
                    .collect()
            };
            assert!(!theirs.is_empty(), "seen from outside");
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(st.outside.as_ref().unwrap().here(), c);
            let p = st.player.as_ref().unwrap();
            let vis = p.vis();
            let lost: Vec<_> = theirs
                .iter()
                .map(|w| handover::local(&st, *w))
                .filter(|q| vis.get(*q).is_none_or(|v| *v == Visibility::Unseen))
                .collect();
            assert!(lost.is_empty(), "forgotten on walking in: {lost:?}");
        }

        #[test]
        fn a_call_holds_you_out_of_the_next_city() {
            let (mut st, mut s, _, x, d) = beside_the_next_city();
            let home = st.outside.as_ref().unwrap().home();
            st.missions.call = Some(ringing(&st));
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(you(&st), x, "not a step in");
            assert_eq!(st.outside.as_ref().unwrap().here(), home);
            assert!(logged(&st, "not with a call on the line"));
            assert_eq!(crate::runs::describe::place_line(&st), "open ground");
        }

        #[test]
        fn a_town_hands_over_too() {
            // from a city's cells onto its town's own tiles inside the city's map: the
            // town takes over at the line, and the cell you stand on is the town's
            let mut st = home_run();
            st.rain = false;
            let (c, town, a, d) = handover::beside_a_town(&mut st);
            handover::on_foot(&mut st, a);
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            assert_eq!(
                st.outside.as_ref().unwrap().here(),
                c,
                "the city's own cell"
            );
            let at = handover::world_of(&st, a.step(d));
            s.update(&mut st, &[key(dir_key(d))]);
            let o = st.outside.as_ref().unwrap();
            assert_eq!(o.here(), town, "the town's");
            let site = st.city.as_ref().unwrap().params.site.clone().unwrap();
            assert_eq!((site.city, site.town), (town.city, town.town));
            assert_eq!(handover::world_of(&st, you(&st)), at);
            assert!(crate::outside::stream::live_owns(&st, you(&st)));
            assert!(
                st.city
                    .as_ref()
                    .unwrap()
                    .walkable(StratumKind::Street, you(&st)),
                "on the town's own street"
            );
        }

        #[test]
        fn the_net_self_walks_into_the_next_city_and_it_hands_over() {
            let (mut st, mut s, c, x, d) = beside_the_next_city();
            jacked_at_a_port(&mut st, &mut s);
            let (place, body) = st.player.as_ref().unwrap().body.unwrap();
            let body_at = handover::world_of(&st, body);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), x);
            assert!(
                net_at(&st, x.step(d))
                    .is_some_and(|t| t.is_walkable() || t.terrain == citygen::Terrain::Void),
                "the next city's Net to walk onto"
            );
            let at = handover::world_of(&st, x.step(d));
            draw_buffer(&s, &st, 120, 40);
            for _ in 0..30 {
                s.update(&mut st, &[]);
            }
            let before = on_screen(&s, &st);
            s.update(&mut st, &[key(dir_key(d))]);
            let o = st.outside.as_ref().unwrap();
            assert_eq!(o.here(), c, "the next city is the live one");
            assert!(logged(&st, &format!("into {}", o.name(c))));
            let p = st.player.as_ref().unwrap();
            assert_eq!(p.place, Place::Stratum(StratumKind::Net));
            assert_eq!(
                handover::world_of(&st, p.pos),
                at,
                "on the cell walked onto"
            );
            assert!(crate::outside::stream::live_owns(&st, p.pos));
            let (now, b) = p.body.expect("still jacked in");
            assert_eq!(
                (now, handover::world_of(&st, b)),
                (place, body_at),
                "the body where it was"
            );
            let (dx, dy) = d.delta();
            assert_eq!(
                on_screen(&s, &st),
                (before.0 + dx, before.1 + dy),
                "nothing on screen jumps"
            );
        }

        #[test]
        fn jacking_out_in_the_next_city_wakes_you_at_the_rig() {
            // jacked in from the flat's rig, the Net self walked into the next city: J
            // hands home back, you in the chair and the screen on the flat
            let (mut st, mut s, c, x, d) = beside_the_next_city();
            let rig = st.home.expect("the flat's rig");
            let city = st.city.clone().unwrap();
            st.interiors
                .ensure(&city, rig.home.building, rig.home.level)
                .expect("the flat's floor");
            handover::jacked(&mut st, (rig.place(), rig.home.rig), x);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), x);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(st.outside.as_ref().unwrap().here(), c, "handed over");
            jack_out(&mut st, &mut s);
            let o = st.outside.as_ref().unwrap();
            assert_eq!(o.here(), o.home(), "home again");
            assert_eq!(player(&st), (rig.place(), rig.home.rig), "at the rig");
            assert_eq!(s.place, rig.place(), "the screen on the flat");
            let p = st.player.as_ref().unwrap();
            assert!(!p.jacked_in());
            assert!(
                p.vis().iter().any(|(_, v)| *v == Visibility::Visible),
                "seeing the flat"
            );
        }

        #[test]
        fn a_fight_holds_your_net_self_at_the_line() {
            let (mut st, mut s, _, x, d) = beside_the_next_city();
            let home = st.outside.as_ref().unwrap().home();
            jacked_at_a_port(&mut st, &mut s);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), x);
            let held = |st: &AppState| {
                assert_eq!(you(st), x, "not a step in");
                assert_eq!(st.outside.as_ref().unwrap().here(), home);
            };
            crate::net::engage(&mut st, Place::Stratum(StratumKind::Net), true);
            s.update(&mut st, &[key(dir_key(d))]);
            held(&st);
            assert!(logged(&st, "not in a fight"), "{:?}", last(&st));
            // a run in flight belongs to this city's Net: not left hanging behind you
            st.net.engaged = None;
            st.net.run = Some(game::Run {
                program: ItemId::new("crack"),
                target: game::HackTarget::Deck,
                done: 0,
                work: 3,
            });
            st.net.beat = st.clock_tick();
            s.update(&mut st, &[key(dir_key(d))]);
            held(&st);
            assert!(logged(&st, "finish the run first"), "{:?}", last(&st));
            // a call on the line
            st.net.run = None;
            st.missions.call = Some(ringing(&st));
            s.update(&mut st, &[key(dir_key(d))]);
            held(&st);
            assert!(
                logged(&st, "not with a call on the line"),
                "{:?}",
                last(&st)
            );
            // nothing holding you: in
            st.missions.call = None;
            s.update(&mut st, &[key(dir_key(d))]);
            assert_ne!(st.outside.as_ref().unwrap().here(), home, "in");
        }

        /// Jacked in at the live city's street port nearest `near`, your body beside the
        /// port, your Net self on `near` and the screen on it.
        fn jacked_near(st: &mut AppState, s: &mut MapScreen, near: Pos) {
            let city = st.city.clone().unwrap();
            let port = city
                .connectors
                .iter()
                .filter(|c| {
                    c.kind == citygen::ConnectorKind::Jack
                        && c.serves.contains(&StratumKind::Street)
                        && city.walkable(StratumKind::Net, c.pos)
                })
                .min_by_key(|c| c.pos.manhattan(near))
                .expect("a street terminal")
                .pos;
            let beside = std::iter::once(port)
                .chain(port.neighbors8())
                .find(|q| city.walkable(StratumKind::Street, *q))
                .unwrap();
            stand(st, s, Place::Stratum(StratumKind::Street), beside);
            st.player.as_mut().unwrap().jack_in(&city, None, 0).unwrap();
            stand(st, s, Place::Stratum(StratumKind::Net), near);
        }

        /// Seed 3's city with a town owning cells of its map, the city live and the
        /// town held (`handover::beside_a_town`), jacked in at the city's street port
        /// nearest the line, your Net self on the city's own Net a step short of the
        /// town's, three steps on into it walkable: the town, the cell and the way in.
        fn short_of_a_town() -> (AppState, MapScreen, Settlement, Pos, Dir) {
            let mut st = home_run();
            st.rain = false;
            let (_, town, a, _) = handover::beside_a_town(&mut st);
            handover::on_foot(&mut st, a);
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            let (x, d) = {
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let net = walked_map(&st, city, Place::Stratum(StratumKind::Net), Some(&b));
                let owner = |q: Pos| crate::outside::stream::owner_of(&st, q);
                TileRect::new(a.x - 24, a.y - 24, 49, 49)
                    .iter()
                    .filter(|q| b.live_owns(*q) && net.walkable(*q))
                    .flat_map(|q| Dir::CARDINAL.into_iter().map(move |d| (q, d)))
                    .filter(|(q, d)| {
                        let (dx, dy) = d.delta();
                        (1..=3).all(|i| {
                            let r = q.offset(dx * i, dy * i);
                            owner(r) == Some(town) && net.walkable(r)
                        })
                    })
                    .min_by_key(|(q, _)| q.manhattan(a))
                    .expect("the city's Net a step short of the town's")
            };
            jacked_near(&mut st, &mut s, x);
            (st, s, town, x, d)
        }

        #[test]
        fn the_cord_follows_you_into_the_next_citys_net() {
            let (mut st, mut s, c, x, d) = short_of_a_town();
            assert_eq!(you(&st), x);
            s.update(&mut st, &[]);
            draw_buffer(&s, &st, 120, 40);
            let v = crate::net::cord_view(&st).expect("the cord");
            let port = handover::world_of(&st, v.port.expect("a port"));
            // three steps onto the next city's Net: it's live, the cord comes along
            for _ in 0..3 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            assert_eq!(st.outside.as_ref().unwrap().here(), c, "handed over");
            let v = crate::net::cord_view(&st).expect("the cord, still");
            assert_eq!(
                v.port.map(|q| handover::world_of(&st, q)),
                Some(port),
                "from the port you jacked in at"
            );
            let me = you(&st);
            let buf = draw_buffer(&s, &st, 120, 40);
            let walk = drawn_cord(&s);
            assert_eq!(
                walk,
                crate::net::cord_walk_beyond(&st, v.entry, me, s.view_rect()),
                "walked in the live city's frame"
            );
            assert_eq!(walk.last().map(|q| q.manhattan(me)), Some(1), "up to you");
            // drawn along all of it the view takes in, whoever's cells it crosses
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            let shown: Vec<(Pos, (u16, u16))> = walk
                .iter()
                .filter_map(|q| tile_to_screen(area, origin, *q).map(|at| (*q, at)))
                .collect();
            assert!(shown.len() > 5, "{} cells of the cord in view", shown.len());
            for (q, at) in &shown {
                assert_eq!(buf[*at].fg, Color::Rgb(230, 240, 245), "the cord at {q:?}");
            }
            assert!(
                shown
                    .iter()
                    .any(|(q, _)| !crate::outside::stream::live_owns(&st, *q)),
                "over the cells you came from"
            );
        }

        /// The cord's cells that lie on a lane (or a gate) of the Net as drawn, the
        /// view put on the one nearest you that `want` takes: each in view keeps the
        /// lane's own glyph, drawn in the cord's colour, as on the live city's own
        /// lanes. How many in view `want` takes.
        fn cord_keeps_the_lanes(
            st: &mut AppState,
            s: &mut MapScreen,
            want: &dyn Fn(&AppState, Pos) -> bool,
        ) -> usize {
            const CORD: Color = Color::Rgb(230, 240, 245);
            draw_buffer(s, st, 120, 40);
            let walk = drawn_cord(s);
            let lane = |st: &AppState, q: Pos| {
                net_at(st, q).is_some_and(|t| {
                    matches!(
                        t.terrain,
                        citygen::Terrain::NetLane | citygen::Terrain::NetGate
                    )
                })
            };
            let me = you(st);
            let Some(look) = walk
                .iter()
                .copied()
                .filter(|q| lane(st, *q) && want(st, *q))
                .min_by_key(|q| q.manhattan(me))
            else {
                return 0;
            };
            s.cam.snap(look);
            st.cord = false;
            let bare = draw_buffer(s, st, 120, 40);
            st.cord = true;
            let corded = draw_buffer(s, st, 120, 40);
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            // walked again for the view there; the port is marked over its lane
            let port = crate::net::cord_view(st).and_then(|v| v.port);
            let mut wanted = 0;
            for q in drawn_cord(s).into_iter().filter(|q| Some(*q) != port) {
                let Some(at) = tile_to_screen(area, origin, q).filter(|_| lane(st, q)) else {
                    continue;
                };
                assert_eq!(
                    corded[at].symbol(),
                    bare[at].symbol(),
                    "the lane's glyph at {q:?}"
                );
                assert_eq!(corded[at].fg, CORD, "the cord's colour at {q:?}");
                wanted += usize::from(want(st, q));
            }
            wanted
        }

        #[test]
        fn the_cord_keeps_a_lanes_glyph_whoever_owns_the_lane() {
            // jacked in at a city's port by the line of a town on its map, into the
            // town's Net: the cord back over the city you came from (held now) keeps its
            // lanes as the live city's own keep theirs
            let (mut st, mut s, town, _, d) = short_of_a_town();
            for _ in 0..3 {
                s.update(&mut st, &[key(dir_key(d))]);
            }
            assert_eq!(st.outside.as_ref().unwrap().here(), town, "handed over");
            let held = cord_keeps_the_lanes(&mut st, &mut s, &|st, q| {
                !crate::outside::stream::live_owns(st, q)
                    && crate::outside::stream::owner_of(st, q).is_some()
            });
            assert!(held > 0, "the cord over the held city's lanes");
            // jacked in by a truck stop's pump, out on the open Net onto the trunk
            // nearest it: its lanes the same
            let mut st = home_run();
            st.rain = false;
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            let pump = by_a_pump(&st);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), pump);
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
            let landed = you(&st);
            crate::outside::stream::tend(
                &mut st,
                TileRect::new(landed.x - 64, landed.y - 64, 129, 129),
            );
            // a cell of a trunk running straight on, some twenty cells off: the cord
            // runs the trunk back
            let open_lane = |q: Pos| {
                crate::outside::stream::owner_of(&st, q).is_none()
                    && net_at(&st, q).is_some_and(|t| t.terrain == citygen::Terrain::NetLane)
            };
            let far = TileRect::new(landed.x - 48, landed.y - 48, 97, 97)
                .iter()
                .filter(|q| {
                    q.manhattan(landed) >= 20
                        && ((-4..=4).all(|i| open_lane(q.offset(i, 0)))
                            || (-4..=4).all(|i| open_lane(q.offset(0, i))))
                })
                .min_by_key(|q| q.manhattan(landed))
                .expect("a trunk by the truck stop");
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), far);
            s.update(&mut st, &[]);
            let open = cord_keeps_the_lanes(&mut st, &mut s, &|st, q| {
                crate::outside::stream::owner_of(st, q).is_none()
            });
            assert!(open > 0, "the cord over the open Net's trunk");
        }

        /// The cord the screen drew last.
        fn drawn_cord(s: &MapScreen) -> Vec<Pos> {
            s.cord_walk
                .borrow()
                .last()
                .expect("the cord drawn")
                .to_vec()
        }

        /// Jacked in at home's street port, the screen drawn: the port.
        fn jacked_at_home() -> (AppState, MapScreen, Pos) {
            let mut st = home_run();
            st.rain = false;
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            jacked_at_a_port(&mut st, &mut s);
            let port = crate::net::port_of(&st).expect("the port");
            (st, s, port)
        }

        /// The open Net's lane cells (on tiles nobody owns) within `r` of `c` (the live
        /// city's frame), built first: the nearest `c` first.
        fn open_lanes_near(st: &mut AppState, c: Pos, r: i32) -> Vec<Pos> {
            let round = TileRect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1);
            crate::outside::stream::tend(st, round);
            let mut lanes: Vec<Pos> = round
                .iter()
                .filter(|q| {
                    crate::outside::stream::owner_of(st, *q).is_none()
                        && net_at(st, *q).is_some_and(|t| t.terrain == citygen::Terrain::NetLane)
                })
                .collect();
            lanes.sort_by_key(|q| q.manhattan(c));
            lanes
        }

        /// A cell of the open Net's trunk at least `min` cells from `from` (the live
        /// city's frame), on the road tile nearest that far out.
        fn trunk_off(st: &mut AppState, from: Pos, min: i32) -> Pos {
            let o = st.outside.as_ref().unwrap();
            let origin = o.origin(st.city.as_ref().unwrap());
            let middle = |t: Pos| {
                Pos::new(
                    TILE * t.x + TILE / 2 - origin.x,
                    TILE * t.y + TILE / 2 - origin.y,
                )
            };
            let tile = o
                .world()
                .tiles
                .iter()
                .filter(|(t, w)| w.road != 0 && o.owner(*t).is_none())
                .map(|(t, _)| t)
                .filter(|t| middle(*t).manhattan(from) >= min)
                .min_by_key(|t| middle(*t).manhattan(from))
                .expect("a trunk that far out");
            let c = middle(tile);
            open_lanes_near(st, c, TILE)[0]
        }

        /// The cord as the screen draws it with you where you stand: a step at a time
        /// up to you, over the Net's lines in view and never the void, its far end out
        /// of sight; and nothing of it drawn off its walk.
        fn the_cord_runs_the_net(st: &mut AppState, s: &mut MapScreen) {
            s.update(st, &[]);
            st.cord = false;
            let bare = draw_buffer(s, st, 120, 40);
            st.cord = true;
            let corded = draw_buffer(s, st, 120, 40);
            let walk = drawn_cord(s);
            let me = you(st);
            assert_eq!(walk.last().map(|q| q.manhattan(me)), Some(1), "up to you");
            assert!(
                walk.windows(2).all(|w| w[0].manhattan(w[1]) == 1),
                "a step at a time"
            );
            let area = s.last_map.get();
            let origin = s.cam.origin(area.width, area.height);
            let at = |q: Pos| tile_to_screen(area, origin, q);
            assert_eq!(at(walk[0]), None, "on out of sight from {:?}", walk[0]);
            let shown: Vec<Pos> = walk.iter().copied().filter(|q| at(*q).is_some()).collect();
            assert!(shown.len() >= 10, "{} cells of it in view", shown.len());
            let void: Vec<Pos> = shown
                .iter()
                .copied()
                .filter(|q| net_at(st, *q).is_none_or(|t| t.terrain == citygen::Terrain::Void))
                .collect();
            assert!(void.is_empty(), "the cord over the void at {void:?}");
            let on: std::collections::BTreeSet<(u16, u16)> =
                shown.iter().filter_map(|q| at(*q)).collect();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    assert!(
                        corded[(x, y)] == bare[(x, y)] || on.contains(&(x, y)),
                        "the cord drawn off its walk at {:?}",
                        (x, y)
                    );
                }
            }
        }

        #[test]
        fn far_out_on_a_trunk_the_cord_runs_the_net_home() {
            // jacked in at home's street port, your Net self out on the open Net's
            // trunk far past the view's reach of the port: the cord runs the trunk back
            // toward it, never straight across the void
            let (mut st, mut s, port) = jacked_at_home();
            let far = trunk_off(&mut st, port, 160);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), far);
            assert!(you(&st).manhattan(port) >= 150, "far out");
            the_cord_runs_the_net(&mut st, &mut s);
        }

        #[test]
        fn the_cord_runs_out_of_a_ports_room() {
            // jacked in at a street port of home's that is a node in a building's room on
            // the Net: a few steps out onto a lane of home's, the cord runs back over the
            // room's floor to it
            let mut st = home_run();
            st.rain = false;
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            jacked_at_a_port_where(&mut st, &mut s, &|city, p| {
                city.tile(StratumKind::Net, p)
                    .is_some_and(|t| t.terrain == citygen::Terrain::NetFloor)
            });
            let port = crate::net::port_of(&st).expect("the port");
            assert_eq!(
                net_at(&st, port).map(|t| t.terrain),
                Some(citygen::Terrain::NetFloor),
                "a port in a room"
            );
            let city = st.city.clone().unwrap();
            let lane = TileRect::new(port.x - 12, port.y - 12, 25, 25)
                .iter()
                .filter(|q| {
                    q.manhattan(port) >= 6
                        && crate::outside::stream::live_owns(&st, *q)
                        && city
                            .tile(StratumKind::Net, *q)
                            .is_some_and(|t| t.terrain == citygen::Terrain::NetLane)
                })
                .min_by_key(|q| q.manhattan(port))
                .expect("a lane by the port");
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), lane);
            draw_buffer(&s, &st, 120, 40);
            let walk = drawn_cord(&s);
            assert_eq!(walk.first(), Some(&port), "from the port");
            assert_eq!(walk.last().map(|q| q.manhattan(lane)), Some(1), "up to you");
        }

        #[test]
        fn in_the_next_citys_net_far_from_home_the_cord_runs_the_net_home() {
            // your Net self handed over into the settlement nearest home, far from
            // home's port, then back out onto the open Net there: the cord runs home
            // over its Net and the trunk, nothing of it left where it ran before
            let (mut st, mut s, port) = jacked_at_home();
            let home = st.outside.as_ref().unwrap().home();
            let (them, lane) = {
                let o = st.outside.as_ref().unwrap();
                let origin = o.origin(st.city.as_ref().unwrap());
                let owns = |s: Settlement, q: Pos| {
                    let c = origin.offset(q.x, q.y);
                    o.owner(Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE))) == Some(s)
                };
                crate::outside::stream::tests::settlements()
                    .iter()
                    .filter(|(t, _)| *t != home)
                    .filter_map(|(t, city)| {
                        let at = o.origin(city);
                        city.stratum(StratumKind::Net)?
                            .tiles
                            .iter()
                            .filter(|(_, x)| x.terrain == citygen::Terrain::NetLane)
                            .map(|(q, _)| handover::local(&st, at.offset(q.x, q.y)))
                            .filter(|q| owns(*t, *q) && q.manhattan(port) >= 150)
                            .min_by_key(|q| q.manhattan(port))
                            .map(|q| (*t, q))
                    })
                    .min_by_key(|(_, q)| q.manhattan(port))
                    .expect("a lane of another settlement's Net far from the port")
            };
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), lane);
            assert_eq!(st.outside.as_ref().unwrap().here(), them, "handed over");
            the_cord_runs_the_net(&mut st, &mut s);
            // out onto the open Net nearest, the settlement still live
            let me = you(&st);
            let out = open_lanes_near(&mut st, me, 4 * TILE)[0];
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), out);
            assert_eq!(st.outside.as_ref().unwrap().here(), them, "still theirs");
            the_cord_runs_the_net(&mut st, &mut s);
        }

        /// A cell of the open Net's lane within four tiles of `near` with `len` cells
        /// of plain lane straight on from it (no lane, gate or node beside them), where
        /// `want` takes the cell and the way: the cell and the way.
        fn a_straight_trunk(
            st: &mut AppState,
            near: Pos,
            len: i32,
            want: &dyn Fn(&AppState, Pos, Dir) -> bool,
        ) -> (Pos, Dir) {
            let lanes = open_lanes_near(st, near, 4 * TILE);
            let lane = |q: Pos| {
                net_at(st, q)
                    .is_some_and(|t| t.terrain == citygen::Terrain::NetLane && t.feature.is_none())
            };
            let side = |q: Pos| {
                net_at(st, q).is_some_and(|t| {
                    matches!(
                        t.terrain,
                        citygen::Terrain::NetLane | citygen::Terrain::NetGate
                    ) || t.feature.is_some()
                })
            };
            lanes
                .iter()
                .find_map(|&from| {
                    Dir::CARDINAL.into_iter().find_map(|d| {
                        let (dx, dy) = d.delta();
                        let straight = (1..=len).all(|i| {
                            let q = from.offset(dx * i, dy * i);
                            lane(q) && !side(q.offset(dy, dx)) && !side(q.offset(-dy, -dx))
                        });
                        (straight && want(st, from, d)).then_some((from, d))
                    })
                })
                .expect("a straight run of trunk")
        }

        /// With the Data surfer talent, a step `d` from `from` on the Net (jacked in),
        /// then a tick at a time `ticks` more: where the ride has you, as a world cell.
        fn ride_from(st: &mut AppState, s: &mut MapScreen, from: Pos, d: Dir, ticks: u64) -> Pos {
            st.character
                .talents
                .insert(game::TalentId::new("data_surfer"), 1);
            stand(st, s, Place::Stratum(StratumKind::Net), from);
            s.update(st, &[key(dir_key(d))]);
            assert!(s.riding.is_some(), "a ride from {from:?}");
            for _ in 0..ticks {
                st.tick += crate::ride::RIDE_TICKS;
                s.update(st, &[]);
            }
            handover::world_of(st, you(st))
        }

        #[test]
        fn a_trunk_rides_as_a_lane_does() {
            // out on the open Net's trunk, a step along it rides on
            let (mut st, mut s, port) = jacked_at_home();
            let far = trunk_off(&mut st, port, 120);
            let (from, d) = a_straight_trunk(&mut st, far, 6, &|_, _, _| true);
            let (dx, dy) = d.delta();
            let six_on = handover::world_of(&st, from.offset(6 * dx, 6 * dy));
            assert_eq!(
                ride_from(&mut st, &mut s, from, d, 5),
                six_on,
                "six cells on"
            );
        }

        #[test]
        fn a_ride_runs_on_as_the_net_past_it_comes_in() {
            // a ride along the trunk longer than the open Net built round you: it stops
            // short where the Net isn't built yet, and rides on once it has come in
            const LONG: usize = 140;
            let (mut st, mut s, port) = jacked_at_home();
            let far = trunk_off(&mut st, port, 140);
            let lanes = open_lanes_near(&mut st, far, 4 * TILE);
            crate::outside::stream::tend(
                &mut st,
                TileRect::new(far.x - 160, far.y - 160, 321, 321),
            );
            let (from, d, whole) = {
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let net = walked_map(&st, city, Place::Stratum(StratumKind::Net), Some(&b));
                lanes
                    .iter()
                    .flat_map(|q| Dir::CARDINAL.into_iter().map(move |d| (*q, d)))
                    .map(|(q, d)| (q, d, crate::ride::ride_path(&net, q, d)))
                    .find(|(_, _, path)| path.len() >= LONG)
                    .expect("a long ride on the trunk")
            };
            st.outside.as_mut().unwrap().stream = Default::default();
            s.last_map.set(Rect::new(0, 0, 8, 4));
            st.character
                .talents
                .insert(game::TalentId::new("data_surfer"), 1);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), from);
            s.update(&mut st, &[key(dir_key(d))]);
            let first = s.riding.as_ref().map_or(0, |r| r.path.len() + 1);
            assert!(
                (1..LONG).contains(&first),
                "cut short at first: {first} cells"
            );
            for _ in 1..LONG {
                st.tick += crate::ride::RIDE_TICKS;
                s.update(&mut st, &[]);
            }
            assert_eq!(you(&st), whole[LONG - 1], "ridden on");
        }

        /// The city or town whose block lies nearest `port` (the live city's frame), and
        /// the middle of the tile of its block nearest the port.
        fn nearest_block(st: &AppState, port: Pos) -> (Settlement, Pos) {
            let o = st.outside.as_ref().unwrap();
            let origin = o.origin(st.city.as_ref().unwrap());
            let pt = origin.offset(port.x, port.y);
            let pt = Pos::new(pt.x.div_euclid(TILE), pt.y.div_euclid(TILE));
            // the tile of its block nearest the port
            let edge = |b: TileRect| {
                Pos::new(
                    pt.x.clamp(b.x, b.x + b.w - 1),
                    pt.y.clamp(b.y, b.y + b.h - 1),
                )
            };
            let (s, t) = o
                .others()
                .into_iter()
                .map(|(s, b)| (s, edge(b)))
                .min_by_key(|(_, t)| t.chebyshev(pt))
                .unwrap();
            let near = Pos::new(
                TILE * t.x + TILE / 2 - origin.x,
                TILE * t.y + TILE / 2 - origin.y,
            );
            (s, near)
        }

        /// The steps `d` from `from` it takes (three at most) to reach a cell another city
        /// or town than the live one owns.
        fn steps_to_their_line(st: &AppState, from: Pos, d: Dir) -> Option<i32> {
            let here = st.outside.as_ref().unwrap().here();
            let (dx, dy) = d.delta();
            (1..=3).find(|i| {
                crate::outside::stream::owner_of(st, from.offset(dx * i, dy * i))
                    .is_some_and(|s| s != here)
            })
        }

        #[test]
        fn a_ride_on_a_trunk_runs_on_into_the_next_citys_net() {
            // a straight run of trunk over the line into the settlement nearest home:
            // the ride hands the run over to it and goes on past the line
            let (mut st, mut s, port) = jacked_at_home();
            let (_, near) = nearest_block(&st, port);
            let theirs = |st: &AppState, q: Pos| crate::outside::stream::owner_of(st, q);
            let line = steps_to_their_line;
            let (from, d) =
                a_straight_trunk(&mut st, near, 6, &|st, from, d| line(st, from, d).is_some());
            let (dx, dy) = d.delta();
            let at = line(&st, from, d).unwrap();
            let them = theirs(&st, from.offset(at * dx, at * dy)).unwrap();
            let six_on = handover::world_of(&st, from.offset(6 * dx, 6 * dy));
            assert_eq!(
                ride_from(&mut st, &mut s, from, d, 5),
                six_on,
                "on past the line"
            );
            assert_eq!(st.outside.as_ref().unwrap().here(), them, "handed over");
        }

        #[test]
        fn a_ride_into_a_net_not_built_yet_keeps_to_its_lanes_once_it_is() {
            // a ride along the trunk into the settlement nearest home before it's built:
            // over its cells the stream stands in for its Net (the trunk running on
            // along the highway) until it is built at the line; from there the ride
            // keeps to its own lanes
            let (mut st, mut s, port) = jacked_at_home();
            let (them, near) = nearest_block(&st, port);
            {
                let o = st.outside.as_mut().unwrap();
                o.stream.finish(&mut o.built);
                o.let_go(them);
                o.stream.on_the_spot(them);
            }
            // their Net as the game builds it, and the world cell of its (0, 0)
            let real = &crate::outside::stream::tests::settlements()
                .iter()
                .find(|(t, _)| *t == them)
                .unwrap()
                .1;
            let origin = st.outside.as_ref().unwrap().origin(real);
            let theirs =
                |w: Pos| real.tile(StratumKind::Net, Pos::new(w.x - origin.x, w.y - origin.y));
            let rides = |w: Pos| {
                theirs(w).is_some_and(|t| {
                    matches!(
                        t.terrain,
                        citygen::Terrain::NetLane | citygen::Terrain::NetGate
                    )
                })
            };
            let walks = |w: Pos| {
                theirs(w).is_some_and(|t| t.is_walkable() || t.terrain == citygen::Terrain::Void)
            };
            let mine = |st: &AppState, w: Pos| {
                let o = st.outside.as_ref().unwrap();
                o.owner(Pos::new(w.x.div_euclid(TILE), w.y.div_euclid(TILE))) == Some(them)
            };
            // a ride from the open Net over their line whose stand-in leaves their lanes
            // for Net you can stand on, every cell of theirs before that walkable
            let lanes = open_lanes_near(&mut st, near, 4 * TILE);
            let (from, d, path) = {
                let city = st.city.as_ref().unwrap();
                let b = Beyond::of(&st).unwrap();
                let net = walked_map(&st, city, Place::Stratum(StratumKind::Net), Some(&b));
                lanes
                    .iter()
                    .flat_map(|q| Dir::CARDINAL.into_iter().map(move |d| (*q, d)))
                    .map(|(q, d)| (q, d, crate::ride::ride_path(&net, q, d)))
                    .find(|(_, _, path)| {
                        let w: Vec<Pos> =
                            path.iter().map(|q| handover::world_of(&st, *q)).collect();
                        let off = w.iter().position(|q| mine(&st, *q) && !rides(*q));
                        off.is_some_and(|i| w[..=i].iter().all(|q| !mine(&st, *q) || walks(*q)))
                    })
                    .expect("a ride whose stand-in leaves their lanes")
            };
            st.character
                .talents
                .insert(game::TalentId::new("data_surfer"), 1);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), from);
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(s.riding.is_some(), "a ride from {from:?}");
            let mut ridden = vec![handover::world_of(&st, you(&st))];
            for _ in 0..path.len() + 2 {
                st.tick += crate::ride::RIDE_TICKS;
                s.update(&mut st, &[]);
                ridden.push(handover::world_of(&st, you(&st)));
            }
            assert_eq!(st.outside.as_ref().unwrap().here(), them, "handed over");
            let off: Vec<Pos> = ridden
                .into_iter()
                .filter(|w| mine(&st, *w) && !rides(*w))
                .collect();
            assert!(off.is_empty(), "ridden off their lanes at {off:?}");
        }

        #[test]
        fn a_ride_found_again_rides_a_loop_of_lane_once() {
            // a ring of lane on open grid of home's Net, ridden round while the ride is
            // found again every frame (as the stream coming in finds it): it goes round
            // once and stops, never on a tile it has ridden
            let (mut st, mut s, port) = jacked_at_home();
            let (w, h) = (8, 5);
            let ring = {
                let city = st.city.as_ref().unwrap();
                let plain = |q: Pos| {
                    crate::outside::stream::live_owns(&st, q)
                        && city.tile(StratumKind::Net, q).is_some_and(|t| {
                            t.terrain == citygen::Terrain::NetGrid && t.feature.is_none()
                        })
                };
                TileRect::new(port.x - 60, port.y - 40, 121, 81)
                    .iter()
                    .map(|q| TileRect::new(q.x, q.y, w, h))
                    .find(|r| r.grow(1).iter().all(plain))
                    .expect("open grid of home's for a ring")
            };
            {
                let net = st
                    .city
                    .as_mut()
                    .unwrap()
                    .stratum_mut(StratumKind::Net)
                    .unwrap();
                for q in ring.border() {
                    net.tiles[q].terrain = citygen::Terrain::NetLane;
                }
            }
            st.character
                .talents
                .insert(game::TalentId::new("data_surfer"), 1);
            let from = Pos::new(ring.x, ring.y);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), from);
            s.update(&mut st, &[key(KeyCode::Char('l'))]);
            assert!(s.riding.is_some(), "a ride round the ring");
            let mut ridden = vec![from, you(&st)];
            for _ in 0..2 * (w + h) {
                st.tick += crate::ride::RIDE_TICKS;
                s.update(&mut st, &[]);
                s.ride_again(&st);
                if you(&st) != *ridden.last().unwrap() {
                    ridden.push(you(&st));
                }
            }
            assert!(s.riding.is_none(), "the ride over");
            let once: std::collections::BTreeSet<Pos> = ridden.iter().copied().collect();
            assert_eq!(once.len(), ridden.len(), "a tile ridden twice: {ridden:?}");
            assert_eq!(ridden.len() as i32, 2 * (w + h) - 4, "round once");
        }

        #[test]
        fn a_ride_stops_at_a_line_it_may_not_cross_and_says_why() {
            // a call comes in as you ride the trunk toward the settlement nearest home:
            // the ride stops a cell short of its line, and says why
            let (mut st, mut s, port) = jacked_at_home();
            let home = st.outside.as_ref().unwrap().here();
            let (_, near) = nearest_block(&st, port);
            let (from, d) = a_straight_trunk(&mut st, near, 6, &|st, from, d| {
                steps_to_their_line(st, from, d).is_some_and(|i| i >= 2)
            });
            let at = steps_to_their_line(&st, from, d).unwrap();
            let (dx, dy) = d.delta();
            let short = handover::world_of(&st, from.offset((at - 1) * dx, (at - 1) * dy));
            st.character
                .talents
                .insert(game::TalentId::new("data_surfer"), 1);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), from);
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(s.riding.is_some(), "a ride from {from:?}");
            st.missions.call = Some(ringing(&st));
            for _ in 0..6 {
                st.tick += crate::ride::RIDE_TICKS;
                s.update(&mut st, &[]);
            }
            assert!(s.riding.is_none(), "the ride over");
            assert_eq!(handover::world_of(&st, you(&st)), short, "held at the line");
            assert_eq!(st.outside.as_ref().unwrap().here(), home, "not handed over");
            assert!(
                logged(&st, "not with a call on the line"),
                "{:?}",
                last(&st)
            );
        }

        /// A cell of another city's Net (held), in the live city's frame, where `want`
        /// says yes, and a cell beside it on the next city's own Net you can stand on:
        /// the cell and the step from beside it.
        fn by_their_net(
            st: &AppState,
            c: Settlement,
            want: &dyn Fn(&citygen::City, Pos) -> bool,
        ) -> (Pos, Dir) {
            let o = st.outside.as_ref().unwrap();
            let them = o.held_city(c).unwrap();
            let at = o.origin(them);
            let b = Beyond::of(st).unwrap();
            let city = st.city.as_ref().unwrap();
            let map = walked_map(st, city, Place::Stratum(StratumKind::Net), Some(&b));
            let mine =
                |q: Pos| matches!(b.elsewhere(q), Some(Owner::City(t, _)) if std::ptr::eq(t, them));
            TileRect::new(0, 0, them.size.w as i32, them.size.h as i32)
                .iter()
                .filter(|q| want(them, *q))
                .map(|q| handover::local(st, at.offset(q.x, q.y)))
                .filter(|p| mine(*p))
                .find_map(|p| {
                    Dir::CARDINAL
                        .into_iter()
                        .find(|d| {
                            let from = p.step(d.opposite());
                            mine(from)
                                && map.walkable(from)
                                && map
                                    .tile(from)
                                    .is_some_and(|t| t.terrain != citygen::Terrain::NetGate)
                        })
                        .map(|d| (p.step(d.opposite()), d))
                })
                .expect("such a cell of their Net")
        }

        #[test]
        fn another_citys_gate_goes_through_once_you_are_on_its_cells() {
            let (mut st, mut s, c, _, _) = beside_the_next_city();
            jacked_at_a_port(&mut st, &mut s);
            let (from, d) = by_their_net(&st, c, &|them, q| {
                them.tile(StratumKind::Net, q)
                    .is_some_and(|t| t.terrain == citygen::Terrain::NetGate && t.building.is_some())
            });
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), from);
            assert_eq!(st.outside.as_ref().unwrap().here(), c, "handed over");
            let was = player(&st);
            s.update(&mut st, &[key(dir_key(d))]);
            assert!(!logged(&st, crate::net::TOO_FAR), "{:?}", last(&st));
            assert_ne!(player(&st), was, "onto their gate: {:?}", last(&st));
        }

        /// A node of held settlement `s`'s put out on its grid on its cell at `at` (the
        /// live city's frame): none is that near on this seed.
        fn put_their_node(st: &mut AppState, s: Settlement, at: Pos) {
            let w = handover::world_of(st, at);
            let o = st.outside.as_mut().unwrap();
            let origin = o.origin(o.held_city(s).unwrap());
            let them = o.built.get_mut(&s).expect("held as built");
            let n = them
                .net
                .nodes
                .iter_mut()
                .find(|n| !matches!(n.kind, NodeKind::Backbone | NodeKind::Ice))
                .expect("a node of theirs");
            n.building = None;
            n.cyber = Pos::new(w.x - origin.x, w.y - origin.y);
        }

        #[test]
        fn a_gate_across_the_line_is_too_far_to_touch() {
            // a gate of the town's on its cell beside the city's own (put there on the
            // town's map): from the city's side of the line it's out of reach
            let mut st = home_run();
            st.rain = false;
            let (c, town, a, d) = handover::beside_a_town(&mut st);
            {
                let w = handover::world_of(&st, a.step(d));
                let o = st.outside.as_mut().unwrap();
                let origin = o.origin(o.held_city(town).unwrap());
                let them = o.built.get_mut(&town).expect("held as built");
                let gate = them
                    .stratum(StratumKind::Net)
                    .unwrap()
                    .tiles
                    .iter()
                    .find_map(|(_, t)| {
                        (t.terrain == citygen::Terrain::NetGate)
                            .then_some(t.building)
                            .flatten()
                    })
                    .expect("a construct's gate");
                let q = Pos::new(w.x - origin.x, w.y - origin.y);
                let net = them.stratum_mut(StratumKind::Net).unwrap();
                net.tiles[q].terrain = citygen::Terrain::NetGate;
                net.tiles[q].building = Some(gate);
            }
            handover::on_foot(&mut st, a);
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            jacked_at_a_port(&mut st, &mut s);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), a);
            assert_eq!(st.outside.as_ref().unwrap().here(), c);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(
                player(&st),
                (Place::Stratum(StratumKind::Net), a),
                "no move"
            );
            assert!(logged(&st, crate::net::TOO_FAR), "{:?}", last(&st));
            assert_eq!(st.outside.as_ref().unwrap().here(), c);
        }

        #[test]
        fn a_node_across_the_line_is_too_far_to_touch() {
            let (mut st, mut s, c, x, d) = beside_the_next_city();
            let home = st.outside.as_ref().unwrap().home();
            st.character = game::Character::debug_runner(&st.content);
            put_their_node(&mut st, c, x.step(d));
            jacked_at_a_port(&mut st, &mut s);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), x);
            assert_eq!(you(&st), x);
            assert_eq!(st.outside.as_ref().unwrap().here(), home, "on the open Net");
            let said = |st: &AppState| {
                st.log
                    .iter()
                    .filter(|e| e.text == crate::net::TOO_FAR)
                    .count()
            };
            // space is still this city's window (Jack out, Look), not their node's
            let cmd = s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(matches!(cmd, ScreenCmd::Push { .. }), "the window");
            assert_eq!(said(&st), 0, "space: {:?}", last(&st));
            // a program at it, from the deck's slots
            for n in '1'..='5' {
                s.update(&mut st, &[key(KeyCode::Char(n))]);
                s.update(&mut st, &[key(KeyCode::Esc)]);
            }
            assert!(said(&st) > 0, "a program: {:?}", last(&st));
            assert_eq!(you(&st), x, "no move");
            // and looked at, space on it
            let before = said(&st);
            s.update(&mut st, &[key(KeyCode::Char('x'))]);
            s.update(&mut st, &[key(dir_key(d))]);
            let cmd = s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(matches!(cmd, ScreenCmd::Stay), "no window");
            assert_eq!(said(&st), before + 1, "look: {:?}", last(&st));
        }

        #[test]
        fn its_ice_and_nodes_answer_after_the_handover() {
            let (mut st, mut s, c, _, _) = beside_the_next_city();
            st.character = game::Character::debug_runner(&st.content);
            jacked_at_a_port(&mut st, &mut s);
            // a node of theirs out on the grid, and a cell of their Net beside it
            let (from, d) = by_their_net(&st, c, &|them, q| {
                them.net.nodes.iter().any(|n| {
                    n.cyber == q
                        && n.building.is_none()
                        && !matches!(n.kind, NodeKind::Backbone | NodeKind::Ice)
                })
            });
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), from);
            assert_eq!(st.outside.as_ref().unwrap().here(), c, "handed over");
            let me = you(&st);
            let node = st
                .city
                .as_ref()
                .unwrap()
                .net
                .nodes
                .iter()
                .find(|n| n.cyber == me.step(d) && n.building.is_none())
                .map(|n| n.id)
                .expect("their node, in their own frame");
            assert!(
                crate::net::targets_outside(&st, me)
                    .iter()
                    .any(|(t, _)| *t == game::HackTarget::Node(node)),
                "their node answers"
            );
            assert!(crate::net::out_of_reach(&st, me).is_empty());
            // a program at it, and looked at: no word of your body
            for n in '1'..='5' {
                s.update(&mut st, &[key(KeyCode::Char(n))]);
                s.update(&mut st, &[key(KeyCode::Esc)]);
            }
            s.update(&mut st, &[key(KeyCode::Char('x'))]);
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(
                s.looking.map(|l| l.cursor),
                Some(me.step(d)),
                "on their node"
            );
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(!logged(&st, crate::net::TOO_FAR), "{:?}", last(&st));
            // their ICE out on their grid, their packets on their lanes
            s.looking = None;
            for _ in 0..3 {
                s.update(&mut st, &[]);
            }
            assert!(st.net.grid.posted, "their sentinels posted");
            assert!(
                st.foes.iter().any(|f| f.ice.is_some() && !f.down),
                "their ICE out"
            );
            draw_buffer(&s, &st, 120, 40);
            assert!(
                s.traffic
                    .borrow()
                    .of_city(st.city.as_ref().unwrap().seed)
                    .is_some_and(|t| t.len() > 0),
                "packets on their lanes"
            );
        }

        #[test]
        fn space_by_another_citys_node_still_opens_the_live_popup() {
            // a node of the town's across the line beside you (put there on the town's
            // map) and a node of the live city's held in reach (put on its own cell
            // nearest): space opens the live city's window, its verbs on it
            let mut st = home_run();
            st.rain = false;
            st.character = game::Character::debug_runner(&st.content);
            let (_, town, a, d) = handover::beside_a_town(&mut st);
            put_their_node(&mut st, town, a.step(d));
            handover::on_foot(&mut st, a);
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            jacked_at_a_port(&mut st, &mut s);
            let spot = {
                let b = Beyond::of(&st).unwrap();
                crate::outside::nearest(a, crate::holds::VERB_REACH, |q| q != a && b.live_owns(q))
                    .expect("the city's own cells in reach")
            };
            let held = {
                let n = st
                    .city
                    .as_mut()
                    .unwrap()
                    .net
                    .nodes
                    .iter_mut()
                    .find(|n| n.kind == NodeKind::Camera)
                    .expect("a camera of the city's");
                n.cyber = spot;
                n.id
            };
            let now = st.net.now(&st);
            let owner = crate::owners::owner_of(st.city.as_ref().unwrap(), held).unwrap();
            crate::holds::take(&mut st, citygen::NetNode::City(held), owner, now);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), a);
            assert!(
                !crate::net::out_of_reach(&st, a).is_empty(),
                "the town's node beside you"
            );
            assert!(
                !interact::verb_rows(&st).is_empty(),
                "the held camera's verbs"
            );
            let cmd = s.update(&mut st, &[key(KeyCode::Char(' '))]);
            assert!(
                matches!(cmd, ScreenCmd::Push { .. }),
                "the window: {:?}",
                last(&st)
            );
            assert!(!logged(&st, crate::net::TOO_FAR));
        }

        #[test]
        fn the_live_citys_nodes_and_gates_answer_only_on_its_own_cells() {
            // a node and a gate of the live city's put on a cell its town owns (no seed
            // has one there yet): the town's map is what's there, so neither answers
            let mut st = home_run();
            st.rain = false;
            let (_, _, a, d) = handover::beside_a_town(&mut st);
            let q = a.step(d);
            let (node, gate) = {
                let city = st.city.as_mut().unwrap();
                let gate = city
                    .stratum(StratumKind::Net)
                    .unwrap()
                    .tiles
                    .iter()
                    .find_map(|(_, t)| {
                        (t.terrain == citygen::Terrain::NetGate)
                            .then_some(t.building)
                            .flatten()
                    })
                    .expect("a construct's gate");
                let n = city
                    .net
                    .nodes
                    .iter_mut()
                    .find(|n| {
                        n.building.is_none()
                            && !matches!(n.kind, NodeKind::Backbone | NodeKind::Ice)
                    })
                    .expect("a node out on the grid");
                n.cyber = q;
                (n.id, gate)
            };
            handover::on_foot(&mut st, a);
            let mut s = MapScreen::walking(&mut st);
            s.update(&mut st, &[]);
            jacked_at_a_port(&mut st, &mut s);
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), a);
            assert!(
                !crate::net::targets_outside(&st, a)
                    .iter()
                    .any(|(t, _)| *t == game::HackTarget::Node(node)),
                "the city's node on the town's cell"
            );
            {
                let net = st
                    .city
                    .as_mut()
                    .unwrap()
                    .stratum_mut(StratumKind::Net)
                    .unwrap();
                net.tiles[q].terrain = citygen::Terrain::NetGate;
                net.tiles[q].building = Some(gate);
            }
            s.update(&mut st, &[key(dir_key(d))]);
            assert_eq!(
                player(&st).0,
                Place::Stratum(StratumKind::Net),
                "not through the city's gate on the town's cell: {:?}",
                last(&st)
            );
        }

        /// The cities next door alive in view (`2026-11-05-streamed-world-design.md`,
        /// "After the first play"): their people's day, and the packets on their lanes
        /// and on the open Net's trunks.
        mod next_door {
            use super::*;
            use std::collections::BTreeMap;

            /// The view the screen is drawn at.
            const SIZE: (u16, u16) = (200, 60);

            /// A cell of the open ground you can stand on a step short of settlement
            /// `c`'s map (`city`), where its walkers on the street (`walking`, cells of
            /// its map) are nearest as a wide view takes them in (a step north or south
            /// counting three), the step onto its cells and how far the nearest walker
            /// is: in the live city's frame.
            fn beside_walkers(
                st: &AppState,
                c: Settlement,
                city: &citygen::City,
                walking: &[Pos],
            ) -> Option<(Pos, Dir, i32)> {
                use citygen::overworld::detail;
                let o = st.outside.as_ref().unwrap();
                let w = o.world();
                let origin = o.origin(city);
                let tile = |p: Pos| Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
                let map = TileRect::new(0, 0, city.size.w as i32, city.size.h as i32);
                let far = |q: Pos| {
                    walking
                        .iter()
                        .map(|b| (b.x - q.x).abs().max(3 * (b.y - q.y).abs()))
                        .min()
                        .unwrap_or(i32::MAX)
                };
                map.border()
                    .flat_map(|q| Dir::CARDINAL.map(|d| (q, d)))
                    .filter(|(q, d)| {
                        let x = origin.offset(q.x, q.y).step(d.opposite());
                        !map.contains(q.step(d.opposite()))
                            && city.walkable(StratumKind::Street, *q)
                            && o.owner(tile(origin.offset(q.x, q.y))) == Some(c)
                            && w.tile(tile(x)).is_some()
                            && o.owner(tile(x)).is_none()
                            && detail::region(w, x.x, x.y, 1, 1)[Pos::new(0, 0)].is_walkable()
                    })
                    .min_by_key(|(q, _)| far(*q))
                    .map(|(q, d)| {
                        let x = origin.offset(q.x, q.y).step(d.opposite());
                        (handover::local(st, x), d, far(q))
                    })
            }

            /// Where the walkers of a city's people `w` stand on its map `city`.
            fn walking(city: &citygen::City, w: &sim::World) -> Vec<Pos> {
                w.visible(city.size.rect())
                    .into_iter()
                    .filter(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
                    .map(|m| m.pos)
                    .collect()
            }

            /// Seed 3's home run at half past seven in the morning (on their way to
            /// work), the neighbour whose people walk nearest an edge of its map with
            /// open ground past it held beside it,
            /// its people seeded as its build seeds them, you on foot on that ground a
            /// step short of its map by its walkers, the screen on you: the run, the
            /// screen, the city, your cell and the step in.
            fn beside_its_people() -> (AppState, MapScreen, Settlement, Pos, Dir) {
                let mut st = AppState::new();
                st.enter_city_at(crate::outside::tests::sited().clone(), 7 * 60 + 30);
                st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
                crate::outside::ensure(&mut st);
                st.rain = false;
                let home = st.outside.as_ref().unwrap().home();
                let (now, minute) = (st.clock_tick(), st.minute_of_day());
                let nearest = crate::outside::stream::tests::settlements()
                    .iter()
                    .filter(|(s, _)| s.town.is_none() && *s != home)
                    .filter_map(|(s, city)| {
                        let w = crate::outside::seed_people(city, &st.people, now, minute);
                        let (.., far) = beside_walkers(&st, *s, city, &walking(city, &w))?;
                        Some((far, *s))
                    })
                    .min_by_key(|(far, s)| (*far, s.city))
                    .map(|(_, s)| s);
                let (c, ..) = handover::beside_a_city_where(&mut st, &|s| Some(s) == nearest);
                handover::seed_its_people(&mut st, c);
                let (x, d) = {
                    let o = st.outside.as_ref().unwrap();
                    let city = o.held_city(c).unwrap();
                    let walking = walking(city, people(&st, c));
                    let (x, d, _) = beside_walkers(&st, c, city, &walking).expect("its edge");
                    (x, d)
                };
                handover::on_foot(&mut st, x);
                let mut s = MapScreen::walking(&mut st);
                draw_buffer(&s, &st, SIZE.0, SIZE.1);
                s.update(&mut st, &[]);
                (st, s, c, x, d)
            }

            /// `n` frames of play, the clock a tick on each, every one drawn: the last.
            fn play(st: &mut AppState, s: &mut MapScreen, n: usize) -> ratatui::buffer::Buffer {
                let mut buf = draw_buffer(s, st, SIZE.0, SIZE.1);
                for _ in 0..n {
                    st.tick += 1;
                    s.update(st, &[]);
                    buf = draw_buffer(s, st, SIZE.0, SIZE.1);
                }
                buf
            }

            /// Settlement `c`'s people, held (not live).
            fn people(st: &AppState, c: Settlement) -> &sim::World {
                crate::outside::people_of(st, c).expect("its people")
            }

            /// Where settlement `c`'s walkers stand on the street, by who: world cells.
            fn walkers(st: &AppState, c: Settlement) -> BTreeMap<sim::PersonId, Pos> {
                let o = st.outside.as_ref().unwrap();
                let city = o.held_city(c).unwrap();
                let at = o.origin(city);
                people(st, c)
                    .visible(city.size.rect())
                    .into_iter()
                    .filter_map(|m| match m.kind {
                        sim::MoverKind::Walker { id, .. } => {
                            Some((id, at.offset(m.pos.x, m.pos.y)))
                        }
                        _ => None,
                    })
                    .collect()
            }

            /// Whether the live city's cell `p` is in sight: on its map by your sight of
            /// it, past it by what you see out there.
            fn in_sight(st: &AppState, p: Pos) -> bool {
                let you = st.player.as_ref().unwrap();
                match you.vis().get(p) {
                    Some(v) => *v == Visibility::Visible,
                    None => you.beyond_sight().binary_search(&p).is_ok(),
                }
            }

            /// The screen cell the live city's cell `p` is drawn on, if the view holds it.
            fn screen_cell(s: &MapScreen, p: Pos) -> Option<(u16, u16)> {
                let area = s.last_map.get();
                tile_to_screen(area, s.cam.origin(area.width, area.height), p)
            }

            #[test]
            fn a_neighbours_people_walk_while_in_view() {
                let (mut st, mut s, c, ..) = beside_its_people();
                let (t0, before) = (people(&st, c).tick(), walkers(&st, c));
                let buf = play(&mut st, &mut s, 3 * sim::TICKS_PER_MINUTE as usize);
                assert_eq!(people(&st, c).tick(), st.clock_tick(), "on the clock");
                assert!(people(&st, c).tick() > t0);
                let after = walkers(&st, c);
                assert!(
                    after.iter().any(|(id, p)| before.get(id) != Some(p)),
                    "they walk on"
                );
                // drawn on the cells it owns that are in sight, as the live city's are
                let mut shown = 0;
                for w in after.values() {
                    let p = handover::local(&st, *w);
                    let Some(at) = screen_cell(&s, p) else {
                        continue;
                    };
                    if crate::outside::stream::owner_of(&st, p) != Some(c) {
                        continue;
                    }
                    let drawn = buf[at].symbol() == "@";
                    if in_sight(&st, p) {
                        assert!(drawn, "a walker of theirs in sight at {w:?}");
                        shown += 1;
                    } else {
                        assert!(!drawn, "a walker of theirs out of sight at {w:?}");
                    }
                }
                assert!(shown > 0, "some of their walkers in sight");
            }

            #[test]
            fn a_neighbour_out_of_view_holds_still() {
                let (mut st, mut s, c, x, _) = beside_its_people();
                // the city or town farthest from you held too, its people seeded
                let far = {
                    let o = st.outside.as_ref().unwrap();
                    let you = handover::world_of(&st, x);
                    let you = Pos::new(you.x.div_euclid(TILE), you.y.div_euclid(TILE));
                    o.others()
                        .into_iter()
                        .filter(|(f, _)| *f != c)
                        .max_by_key(|(_, b)| Pos::new(b.x, b.y).manhattan(you))
                        .map(|(f, _)| f)
                        .unwrap()
                };
                handover::hold(&mut st, far);
                handover::seed_its_people(&mut st, far);
                let (t0, before) = (people(&st, far).tick(), walkers(&st, far));
                play(&mut st, &mut s, 3 * sim::TICKS_PER_MINUTE as usize);
                assert_eq!(people(&st, far).tick(), t0, "held still");
                assert_eq!(walkers(&st, far), before);
                assert_eq!(
                    people(&st, c).tick(),
                    st.clock_tick(),
                    "the one in view runs"
                );
            }

            #[test]
            fn handing_over_keeps_its_people_where_they_were_drawn() {
                let (mut st, mut s, c, _, d) = beside_its_people();
                play(&mut st, &mut s, 2 * sim::TICKS_PER_MINUTE as usize);
                let before = walkers(&st, c);
                let (tick, count) = (people(&st, c).tick(), people(&st, c).people().len());
                assert!(!before.is_empty());
                // the step in, on the same tick
                s.update(&mut st, &[key(dir_key(d))]);
                assert_eq!(st.outside.as_ref().unwrap().here(), c, "walked in");
                let w = st.world.as_ref().unwrap();
                assert_eq!(w.tick(), tick, "nothing to catch up");
                assert_eq!(w.people().len(), count, "not seeded again");
                let at = {
                    let o = st.outside.as_ref().unwrap();
                    o.origin(st.city.as_ref().unwrap())
                };
                // you on the cell stepped onto; whoever stood there is held a step back
                let you = handover::world_of(&st, you(&st));
                for (id, p) in before.iter().filter(|(_, p)| **p != you) {
                    assert_eq!(
                        w.pos_of(*id).map(|q| at.offset(q.x, q.y)),
                        Some(*p),
                        "{id:?} where they were drawn"
                    );
                }
            }

            /// The cells of `traffic`'s lanes a packet lights at `tick`, each with whether
            /// it's the head: in the live city's frame, its own frame `shift` cells off.
            fn lit(
                traffic: &crate::render::netlook::packets::Traffic,
                tick: u64,
                shift: Pos,
            ) -> Vec<(Pos, bool)> {
                traffic
                    .glows(tick)
                    .into_iter()
                    .map(|(p, (_, head))| (Pos::new(p.x - shift.x, p.y - shift.y), head))
                    .collect()
            }

            /// The lane cells a Net's packets light this frame, each with whether it's
            /// the head: cells of the live city's frame.
            type Lit<'a> = dyn Fn(&AppState, &MapScreen) -> Vec<(Pos, bool)> + 'a;

            /// Over `n` frames of play, how many times a packet's head was drawn (white
            /// on its lane) on a cell `mine` takes, of those `lit` gives each frame: not
            /// where your cord or you are drawn over it.
            fn heads_drawn(
                st: &mut AppState,
                s: &mut MapScreen,
                n: usize,
                lit: &Lit<'_>,
                mine: &dyn Fn(&AppState, Pos) -> bool,
            ) -> usize {
                let mut heads = 0;
                for _ in 0..n {
                    let buf = play(st, s, 1);
                    let over: Vec<Pos> = s
                        .cord_walk
                        .borrow()
                        .last()
                        .unwrap_or_default()
                        .iter()
                        .copied()
                        .chain([you(st)])
                        .collect();
                    for (p, head) in lit(st, s) {
                        let Some(at) =
                            screen_cell(s, p).filter(|_| head && mine(st, p) && !over.contains(&p))
                        else {
                            continue;
                        };
                        assert_eq!(buf[at].fg, Color::Rgb(255, 255, 255), "a head at {p:?}");
                        heads += 1;
                    }
                }
                heads
            }

            #[test]
            fn a_neighbours_lanes_carry_packets() {
                let (mut st, mut s, c, x, d) = beside_its_people();
                jacked_near(&mut st, &mut s, x);
                assert_ne!(st.outside.as_ref().unwrap().here(), c, "beside it");
                let seed = st.outside.as_ref().unwrap().held_city(c).unwrap().seed;
                let lit = |st: &AppState, s: &MapScreen| {
                    let o = st.outside.as_ref().unwrap();
                    let (live, theirs) = (
                        o.origin(st.city.as_ref().unwrap()),
                        o.origin(o.held_city(c).unwrap()),
                    );
                    let shift = Pos::new(live.x - theirs.x, live.y - theirs.y);
                    let nets = s.traffic.borrow();
                    let traffic = nets.of_city(seed).expect("its own traffic");
                    lit(traffic, st.tick, shift)
                };
                let theirs = |st: &AppState, p: Pos| {
                    crate::outside::stream::owner_of(st, p) == Some(c)
                        && !crate::outside::stream::live_owns(st, p)
                };
                let heads = heads_drawn(&mut st, &mut s, 90, &lit, &theirs);
                assert!(heads > 0, "packets run on its lanes");
                // your Net self onto its Net on the same tick: live now, its packets
                // where they were (world cells)
                let world = |st: &AppState, lit: Vec<(Pos, bool)>| {
                    lit.into_iter()
                        .map(|(p, head)| (handover::world_of(st, p), head))
                        .collect::<std::collections::BTreeSet<_>>()
                };
                let before = world(&st, lit(&st, &s));
                assert!(!before.is_empty());
                s.update(&mut st, &[key(dir_key(d))]);
                assert_eq!(st.outside.as_ref().unwrap().here(), c, "handed over");
                draw_buffer(&s, &st, SIZE.0, SIZE.1);
                let after = {
                    let nets = s.traffic.borrow();
                    let traffic = nets.of_city(seed).expect("its own traffic, still");
                    world(&st, self::lit(traffic, st.tick, Pos::new(0, 0)))
                };
                assert_eq!(after, before, "its packets carry on");
            }

            #[test]
            fn the_open_nets_trunks_carry_packets() {
                let mut st = home_run();
                st.rain = false;
                let mut s = MapScreen::walking(&mut st);
                s.update(&mut st, &[]);
                jacked_at_a_port(&mut st, &mut s);
                // a cell of a trunk on the open ground, its lane running straight on 24
                // cells each way and no node of the open Net's anywhere near (its
                // packets from no node's), nearest the city
                let on = {
                    let city = st.city.as_ref().unwrap();
                    let (w, h) = (city.size.w as i32, city.size.h as i32);
                    crate::outside::stream::tend(
                        &mut st,
                        TileRect::new(-160, -80, w + 320, h + 160),
                    );
                    let o = st.outside.as_ref().unwrap();
                    let origin = o.origin(st.city.as_ref().unwrap());
                    let open_lane = |p: Pos| {
                        crate::outside::stream::owner_of(&st, p).is_none()
                            && o.stream
                                .net
                                .tile(origin.offset(p.x, p.y))
                                .is_some_and(|t| t.terrain == citygen::Terrain::NetLane)
                    };
                    let middle = Pos::new(w / 2, h / 2);
                    let around = TileRect::new(-160, -80, w + 320, h + 160);
                    let nodes: Vec<Pos> = o
                        .stream
                        .net
                        .cells_in(TileRect::new(
                            around.x + origin.x,
                            around.y + origin.y,
                            around.w,
                            around.h,
                        ))
                        .filter(|(_, t)| matches!(t.feature, Some(citygen::Feature::Node(_))))
                        .map(|(c, _)| Pos::new(c.x - origin.x, c.y - origin.y))
                        .collect();
                    TileRect::new(-150, -70, w + 300, h + 140)
                        .iter()
                        .filter(|p| {
                            (-24..=24).all(|i| open_lane(p.offset(i, 0)))
                                || (-12..=12).all(|i| open_lane(p.offset(0, i)))
                        })
                        .filter(|p| {
                            nodes
                                .iter()
                                .all(|n| (n.x - p.x).abs() > 140 || (n.y - p.y).abs() > 50)
                        })
                        .min_by_key(|p| p.manhattan(middle))
                        .expect("a trunk on the open ground")
                };
                stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), on);
                assert!(st.player.as_ref().unwrap().jacked_in());
                let lit = |st: &AppState, s: &MapScreen| {
                    let o = st.outside.as_ref().unwrap();
                    let live = o.origin(st.city.as_ref().unwrap());
                    let nets = s.traffic.borrow();
                    let traffic = nets.open_net().expect("the open Net's traffic");
                    lit(traffic, st.tick, live)
                };
                let open = |st: &AppState, p: Pos| {
                    crate::outside::stream::owner_of(st, p).is_none()
                        && !crate::outside::stream::live_owns(st, p)
                };
                let heads = heads_drawn(&mut st, &mut s, 90, &lit, &open);
                assert!(heads > 0, "packets run on the trunks");
            }
        }

        /// The long walk: out across the open ground and back on foot, then jacked in
        /// at a ground port and out across the open Net and back, the clock running all
        /// the way and every frame drawn at three sizes; four times on each walk out,
        /// every window the map opens and every key its help lists.
        mod long_walk {
            use super::*;
            use crate::screens::deck::Tab;
            use crossterm::event::KeyEvent;

            /// The sizes every frame is drawn at.
            const SIZES: [(u16, u16); 3] = [(80, 24), (120, 40), (200, 60)];
            /// Steps out, and as many back.
            const STEPS: usize = 120;
            /// Where on the way out every window and key is tried.
            const STOPS: [usize; 4] = [0, 40, 80, 120];

            /// The screens as the app stacks them: the city screen, or the overworld a
            /// `W` switched to in its place, and what was pushed over either.
            struct Stack {
                map: MapScreen,
                up: Option<Box<dyn Screen>>,
                over: Vec<Box<dyn Screen>>,
            }

            impl Stack {
                fn new(st: &mut AppState) -> Stack {
                    let mut t = Stack {
                        map: MapScreen::walking(st),
                        up: None,
                        over: Vec::new(),
                    };
                    t.frame(st, &[]);
                    t
                }

                fn depth(&self) -> usize {
                    1 + self.over.len()
                }

                /// One frame as the app runs it: the clock a tick on, the keys to the
                /// top screen, what it asks done, the whole stack drawn at every size.
                fn frame(&mut self, st: &mut AppState, keys: &[KeyEvent]) {
                    st.tick += 1;
                    let top: &mut dyn Screen = match (self.over.last_mut(), self.up.as_mut()) {
                        (Some(s), _) => s.as_mut(),
                        (None, Some(s)) => s.as_mut(),
                        (None, None) => &mut self.map,
                    };
                    match top.update(st, keys) {
                        ScreenCmd::Stay | ScreenCmd::Flash(_) => {}
                        ScreenCmd::Push { to, .. } => self.over.push(to),
                        ScreenCmd::Pop { .. } => {
                            self.over.pop();
                        }
                        ScreenCmd::Replace { to, .. } => {
                            self.over.pop();
                            self.over.push(to);
                        }
                        // down from the overworld: the city screen, as a run resumes it
                        ScreenCmd::Switch { .. } if self.up.is_some() => {
                            self.over.clear();
                            self.up = None;
                            self.map = MapScreen::resumed(st);
                        }
                        ScreenCmd::Switch { to, .. } => {
                            self.over.clear();
                            self.up = Some(to);
                        }
                        ScreenCmd::Quit => panic!("the game quit"),
                    }
                    self.draw(st);
                }

                fn draw(&self, st: &AppState) {
                    for (w, h) in SIZES {
                        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                        term.draw(|f| {
                            match &self.up {
                                Some(s) => s.draw(st, f),
                                None => self.map.draw(st, f),
                            }
                            for s in &self.over {
                                crate::ui::dim::wash(f.buffer_mut());
                                s.draw(st, f);
                            }
                        })
                        .unwrap();
                    }
                }

                fn press(&mut self, st: &mut AppState, code: KeyCode) {
                    self.frame(st, &[key(code)]);
                }

                /// Esc until only the map is left, and a pick or a target put away.
                fn close(&mut self, st: &mut AppState) {
                    for _ in 0..6 {
                        if self.depth() == 1 {
                            break;
                        }
                        self.press(st, KeyCode::Esc);
                    }
                    assert_eq!(self.depth(), 1, "put away: {:?}", last(st));
                    if self.map.which_way.is_some() || self.map.targeting.is_some() {
                        self.press(st, KeyCode::Esc);
                    }
                }
            }

            /// One step `d` from `from` on `place`, frames apart as a player's keys are,
            /// a minute of the clock run on every tenth: you're where the step says.
            fn go(st: &mut AppState, t: &mut Stack, place: Place, from: Pos, d: Dir, i: usize) {
                st.tick += sim::WALK_TICKS;
                // the arrows on every other step, the letters on the rest
                let arrows = i.is_multiple_of(2);
                let code = match d {
                    Dir::N if arrows => KeyCode::Up,
                    Dir::S if arrows => KeyCode::Down,
                    Dir::E if arrows => KeyCode::Right,
                    Dir::W if arrows => KeyCode::Left,
                    _ => dir_key(d),
                };
                t.press(st, code);
                assert_eq!(
                    player(st),
                    (place, from.step(d)),
                    "step {i} {d:?}: {:?}",
                    last(st)
                );
                if i % 10 == 9 {
                    crate::clock::run(
                        st,
                        Some(sim::TICKS_PER_MINUTE),
                        crate::clock::PATHS_PER_FRAME,
                    );
                }
            }

            /// `STEPS` steps of a seeded wander from where you stand, only onto cells of
            /// the open ground the map you're on lets you onto, two in three away from
            /// the city's middle; everything tried at each of `STOPS`. The steps taken.
            fn wander(st: &mut AppState, t: &mut Stack, seed: u64) -> Vec<Dir> {
                let mut taken = Vec::new();
                for i in 0..=STEPS {
                    if STOPS.contains(&i) {
                        everything(st, t);
                    }
                    if i == STEPS {
                        break;
                    }
                    let (place, from) = player(st);
                    let d = {
                        let city = st.city.as_ref().unwrap();
                        let b = Beyond::of(st).unwrap();
                        let map = walked_map(st, city, place, Some(&b));
                        let open: Vec<Dir> = Dir::ALL
                            .into_iter()
                            .filter(|d| {
                                let to = from.step(*d);
                                // the open ground's own, never a city's or a town's
                                // (a step there would hand the run over)
                                map.walkable(to)
                                    && matches!(b.elsewhere(to), Some(Owner::Ground(..)))
                                    && crate::outside::stream::owner_of(st, to).is_none()
                            })
                            .collect();
                        assert!(!open.is_empty(), "step {i}: nowhere to go from {from:?}");
                        let mid = city.size.center();
                        let h = hash64(seed, i as u64);
                        if h.is_multiple_of(3) {
                            open[(h / 3) as usize % open.len()]
                        } else {
                            *open
                                .iter()
                                .max_by_key(|d| from.step(**d).dist2(mid))
                                .unwrap()
                        }
                    };
                    go(st, t, place, from, d, i);
                    taken.push(d);
                }
                taken
            }

            /// The way back, step for step.
            fn back(st: &mut AppState, t: &mut Stack, taken: &[Dir]) {
                for (i, d) in taken.iter().rev().enumerate() {
                    let (place, from) = player(st);
                    go(st, t, place, from, d.opposite(), i);
                }
            }

            /// Every window the map opens and every key its help lists, each put away
            /// again, a frame drawn after every key: you end where you were.
            fn everything(st: &mut AppState, t: &mut Stack) {
                let was = player(st);
                let jacked = st.player.as_ref().unwrap().jacked_in();
                // the deck on every tab, ATTRIBUTES the sheet, the MAP at every zoom
                t.press(st, KeyCode::Tab);
                assert_eq!(t.depth(), 2, "the deck: {:?}", last(st));
                for (n, tab) in Tab::ALL.iter().enumerate() {
                    t.press(
                        st,
                        KeyCode::Char(char::from_digit(n as u32 + 1, 10).unwrap()),
                    );
                    for k in [
                        KeyCode::Down,
                        KeyCode::Down,
                        KeyCode::Up,
                        KeyCode::Right,
                        KeyCode::Left,
                    ] {
                        t.press(st, k);
                    }
                    if *tab == Tab::Map {
                        // in to zoom 1, then out a step at a time to the world
                        for _ in 0..3 {
                            t.press(st, KeyCode::Char('+'));
                        }
                        for _ in 0..4 {
                            for c in "hlkjHLKJ[]<>fft".chars() {
                                t.press(st, KeyCode::Char(c));
                            }
                            t.press(st, KeyCode::Char('-'));
                        }
                        // on the world: a place marked and unmarked, then back in
                        for c in "]tt+".chars() {
                            t.press(st, KeyCode::Char(c));
                        }
                    }
                }
                t.press(st, KeyCode::Tab);
                assert_eq!(t.depth(), 1, "the deck put away");
                // the panes, the inspector and the legend among them, each off and on
                for n in 1..=6 {
                    t.press(st, KeyCode::F(n));
                    t.press(st, KeyCode::F(n));
                }
                // look mode, the inspector reading the cursor
                st.sidebar.set(Section::Inspect, true);
                t.press(st, KeyCode::Char('x'));
                assert!(t.map.looking.is_some(), "look mode");
                for c in "hhhllllllkkkjjjjjjyubnHJKLYUBN".chars() {
                    t.press(st, KeyCode::Char(c));
                    let l = t.map.looking.expect("still looking");
                    assert!(
                        t.map.view_rect().contains(l.cursor),
                        "the look cursor {:?} out of view {:?}, you at {:?}, cam {:?}",
                        l.cursor,
                        t.map.view_rect(),
                        player(st),
                        (t.map.cam.x, t.map.cam.y)
                    );
                }
                for k in [
                    KeyCode::Tab,
                    KeyCode::Tab,
                    KeyCode::BackTab,
                    KeyCode::Char(' '),
                ] {
                    t.press(st, k);
                }
                t.close(st);
                if t.map.looking.is_some() {
                    t.press(st, KeyCode::Esc);
                }
                assert!(t.map.looking.is_none(), "look put away");
                // the windows the keys push, each put away
                for c in ['v', 'm', 'a', 'q'] {
                    t.press(st, KeyCode::Char(c));
                    t.close(st);
                }
                t.press(st, KeyCode::Esc);
                assert_eq!(t.depth(), 2, "esc pauses");
                t.close(st);
                // space: what's in reach (a port beside you on foot jacks you in: out)
                t.press(st, KeyCode::Char(' '));
                if !jacked && st.player.as_ref().unwrap().jacked_in() {
                    t.press(st, KeyCode::Char('J'));
                    t.press(st, KeyCode::Char('y'));
                }
                t.close(st);
                // on and off again: help, the digital overlay, dev mode
                for c in ['?', 'd', '`'] {
                    t.press(st, KeyCode::Char(c));
                    t.press(st, KeyCode::Char(c));
                }
                // stairs, a wait, engage, aim, save
                for c in ['<', '>', '.', 'e', 'g', 's'] {
                    t.press(st, KeyCode::Char(c));
                    if st.net.engaged.is_some() {
                        t.press(st, KeyCode::Char('e'));
                    }
                    t.close(st);
                }
                // the slots: an ability on foot, a program jacked in
                for c in '1'..='5' {
                    t.press(st, KeyCode::Char(c));
                    t.close(st);
                }
                // J asks jacked in, and you stay
                t.press(st, KeyCode::Char('J'));
                if jacked {
                    assert_eq!(t.depth(), 2, "J asks");
                    t.press(st, KeyCode::Char('n'));
                }
                assert_eq!(t.depth(), 1);
                // the free camera, and back on foot
                t.press(st, KeyCode::Char('c'));
                assert_eq!(t.map.mode, Mode::Explore);
                for c in "hjkl[]".chars() {
                    t.press(st, KeyCode::Char(c));
                }
                t.press(st, KeyCode::Char('w'));
                assert_eq!(t.map.mode, Mode::Walk);
                // the console
                t.press(st, KeyCode::Char(':'));
                assert!(t.map.console.is_some(), "the console");
                t.press(st, KeyCode::Esc);
                assert!(t.map.console.is_none());
                // up: the overworld on foot, the Net over the world jacked in; and down
                t.press(st, KeyCode::Char('W'));
                if jacked {
                    assert_eq!(t.depth(), 2, "W looks up at the Net: {:?}", last(st));
                } else {
                    assert!(t.up.is_some(), "W goes up: {:?}", last(st));
                }
                t.press(st, KeyCode::Char('?'));
                t.press(st, KeyCode::Char('?'));
                // jacked in, a tile out over the Net and back, and down
                if jacked {
                    t.press(st, KeyCode::Char('l'));
                    t.press(st, KeyCode::Char('h'));
                }
                t.press(st, KeyCode::Enter);
                assert!(t.up.is_none() && t.depth() == 1, "back down");
                // ↵ came down on the tile's road: back where the walk is
                if player(st) != was {
                    stand(st, &mut t.map, was.0, was.1);
                }
                assert_eq!(player(st), was, "{:?}", last(st));
                assert_eq!(t.map.mode, Mode::Walk);
            }

            #[test]
            fn a_long_walk_out_and_back_never_breaks() {
                let (mut st, _, e, _) = at_the_edge(1);
                st.rain = true;
                st.unlock_dev();
                st.character = game::Character::debug_runner(&st.content);
                crate::bots::sync(&mut st);
                let street = Place::Stratum(StratumKind::Street);
                let mut t = Stack::new(&mut st);
                let minutes =
                    |st: &AppState, from: u64| (st.clock_tick() - from) / sim::TICKS_PER_MINUTE;
                // on foot: out across the ground, and back the way you came
                let from = st.clock_tick();
                let out = wander(&mut st, &mut t, 3);
                assert!(off_the_map(&st, you(&st)));
                back(&mut st, &mut t, &out);
                assert_eq!(player(&st), (street, e), "back where the walk began");
                assert!(
                    minutes(&st, from) >= 30,
                    "the clock ran: {}",
                    minutes(&st, from)
                );
                // by a pump on the ground: jacked in, out across the open Net and back
                let port = by_a_pump(&st);
                stand(&mut st, &mut t.map, street, port);
                t.press(&mut st, KeyCode::Char(' '));
                assert!(
                    st.player.as_ref().unwrap().jacked_in(),
                    "jacked in: {:?}",
                    last(&st)
                );
                let start = player(&st);
                // a mask on, wearing out on the grid, and a tracer on your trail
                st.net.mask = 6;
                let d = st.city.as_ref().unwrap().districts[0].id;
                crate::grid::send_tracer(&mut st, crate::owners::Owner::District(d));
                let from = st.clock_tick();
                let out = wander(&mut st, &mut t, 4);
                assert!(off_the_map(&st, you(&st)));
                back(&mut st, &mut t, &out);
                assert_eq!(player(&st), start, "back where the Net walk began");
                assert!(
                    minutes(&st, from) >= 30,
                    "the clock ran: {}",
                    minutes(&st, from)
                );
                // and out of the Net, back in the body by the pump
                t.press(&mut st, KeyCode::Char('J'));
                t.press(&mut st, KeyCode::Char('y'));
                assert_eq!(t.depth(), 1);
                assert_eq!(player(&st), (street, port), "in the body: {:?}", last(&st));
            }
        }
    }

    /// One light (`2026-11-05-streamed-world-design.md`, "After the first play"): a
    /// city's cells look the same drawn from outside it as from inside, at the same hour,
    /// people aside.
    mod one_light {
        use super::*;
        use crate::outside::stream::tests::{handover, settlements};
        use crate::outside::{Settlement, TILE};
        use std::collections::{BTreeMap, BTreeSet};

        /// The view drawn: wide enough to take in a patch of a city from beside it.
        const SIZE: (u16, u16) = (160, 50);

        /// Noon, dusk (the lamps just lit) and midnight.
        const HOURS: [(u16, &str); 3] = [(12 * 60, "noon"), (18 * 60, "dusk"), (0, "midnight")];

        /// What a world cell looks like on screen: glyph, foreground, background and
        /// modifiers.
        type Look = (String, Color, Color, Modifier);

        /// The world cells of a view, as drawn plainly and in the dev light view (`light`).
        struct View {
            plain: BTreeMap<Pos, Look>,
            light: BTreeMap<Pos, Look>,
        }

        /// Seed 3's home city on its site at `minute` of the day, its world made, every
        /// other city and town of it held (built as the game builds them); rain as play
        /// has it.
        fn run_at(minute: u16) -> AppState {
            let mut st = AppState::new();
            st.enter_city_at(crate::outside::tests::sited().clone(), minute);
            st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
            crate::outside::ensure(&mut st);
            let o = st.outside.as_mut().unwrap();
            let home = o.home();
            for (s, city) in settlements() {
                if *s != home {
                    o.built.insert(*s, city.clone());
                }
            }
            st
        }

        fn tile(c: Pos) -> Pos {
            Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE))
        }

        /// `city`'s street cells in buildings.
        fn built(city: &citygen::City) -> Vec<Pos> {
            city.stratum(StratumKind::Street)
                .unwrap()
                .tiles
                .iter()
                .filter(|(_, t)| t.building.is_some())
                .map(|(p, _)| p)
                .collect()
        }

        /// A walkable cell of the open ground beside a street cell settlement `s` owns on
        /// its map (`city`), nearest its cells `near`, and a cell of its own ten steps
        /// further in: world cells. None where no open ground touches it.
        fn beside(
            st: &AppState,
            s: Settlement,
            city: &citygen::City,
            near: &[Pos],
        ) -> Option<(Pos, Pos)> {
            use citygen::overworld::detail;
            let o = st.outside.as_ref().unwrap();
            let w = o.world();
            let origin = o.origin(city);
            let near = |q: Pos| {
                near.iter()
                    .map(|b| (b.x - q.x).abs().max(2 * (b.y - q.y).abs()))
                    .min()
                    .unwrap_or(i32::MAX)
            };
            let mut edge: Vec<(i32, Pos, Pos)> =
                TileRect::new(0, 0, city.size.w as i32, city.size.h as i32)
                    .iter()
                    .filter(|q| {
                        o.owner(tile(origin.offset(q.x, q.y))) == Some(s)
                            && city.walkable(StratumKind::Street, *q)
                    })
                    .flat_map(|q| {
                        q.neighbors4()
                            .map(move |n| (q, origin.offset(n.x, n.y)))
                            .collect::<Vec<_>>()
                    })
                    .filter(|(_, x)| w.tile(tile(*x)).is_some() && o.owner(tile(*x)).is_none())
                    .map(|(q, x)| (near(q), origin.offset(q.x, q.y), x))
                    .collect();
            edge.sort();
            edge.into_iter()
                .find(|(_, _, x)| detail::region(w, x.x, x.y, 1, 1)[Pos::new(0, 0)].is_walkable())
                .map(|(_, y, x)| (x, Pos::new(y.x + 10 * (y.x - x.x), y.y + 10 * (y.y - x.y))))
        }

        /// The world cells settlement `s` owns on its map (`city`).
        fn owned_by(
            st: &AppState,
            s: Settlement,
            city: &citygen::City,
        ) -> impl Fn(Pos) -> bool + use<> {
            let o = st.outside.as_ref().unwrap();
            let origin = o.origin(city);
            let size = city.size;
            let tiles: BTreeSet<Pos> = TileRect::new(0, 0, size.w as i32, size.h as i32)
                .iter()
                .map(|q| tile(origin.offset(q.x, q.y)))
                .filter(|t| o.owner(*t) == Some(s))
                .collect();
            move |c: Pos| {
                size.contains(Pos::new(c.x - origin.x, c.y - origin.y)) && tiles.contains(&tile(c))
            }
        }

        /// The world cells the live city's people and your bots stand on, or light (the
        /// doors open buildings light by night): what isn't the city's own look.
        fn people(st: &AppState) -> BTreeSet<Pos> {
            let (Some(city), Some(world), Some(o)) =
                (st.city.as_ref(), st.world.as_ref(), st.outside.as_ref())
            else {
                return BTreeSet::new();
            };
            let origin = o.origin(city);
            let all = TileRect::new(
                -200,
                -200,
                city.size.w as i32 + 400,
                city.size.h as i32 + 400,
            );
            world
                .visible(all)
                .into_iter()
                .map(|m| m.pos)
                .chain(world.open_buildings().flat_map(|b| world.doors(b).to_vec()))
                .chain(st.bots.iter().filter_map(|b| b.pos))
                .chain(st.bots.iter().flat_map(|b| b.wake.iter().map(|(q, _)| *q)))
                .map(|p| origin.offset(p.x, p.y))
                .collect()
        }

        /// You on foot on the street at world cell `at` (having looked round from world
        /// cell `from` first, when given), the screen on you at rest: every world cell of
        /// the view `keep` takes, as drawn.
        fn drawn(
            st: &mut AppState,
            from: Option<Pos>,
            at: Pos,
            keep: &dyn Fn(Pos) -> bool,
        ) -> View {
            if let Some(from) = from {
                handover::on_foot(st, handover::local(st, from));
                look_round(st);
            }
            handover::on_foot(st, handover::local(st, at));
            look_round(st);
            if crate::outside::stream::finish(st) {
                look_round(st);
            }
            let mut s = MapScreen::resumed(st);
            for _ in 0..30 {
                s.update(st, &[]);
            }
            let look = |st: &AppState| {
                let buf = draw_buffer(&s, st, SIZE.0, SIZE.1);
                let area = s.last_map.get();
                let origin = s.cam.origin(area.width, area.height);
                let corner = {
                    let o = st.outside.as_ref().unwrap();
                    o.origin(st.city.as_ref().unwrap())
                };
                area.positions()
                    .filter_map(|c| {
                        let p = crate::render::viewport::screen_to_tile(area, origin, (c.x, c.y));
                        let w = corner.offset(p.x, p.y);
                        let cell = &buf[(c.x, c.y)];
                        keep(w).then(|| {
                            (
                                w,
                                (cell.symbol().to_string(), cell.fg, cell.bg, cell.modifier),
                            )
                        })
                    })
                    .collect::<BTreeMap<_, _>>()
            };
            let plain = look(st);
            st.show_light = true;
            let light = look(st);
            st.show_light = false;
            View { plain, light }
        }

        /// Every cell drawn in both that isn't `aside`, where they differ: how many were
        /// compared, and each that differs with how it looked from outside and inside.
        fn differences(
            outside: &BTreeMap<Pos, Look>,
            inside: &BTreeMap<Pos, Look>,
            aside: &BTreeSet<Pos>,
        ) -> (usize, Vec<(Pos, Look, Look)>) {
            let mut both = 0;
            let mut out = Vec::new();
            for (w, a) in outside {
                let Some(b) = inside.get(w).filter(|_| !aside.contains(w)) else {
                    continue;
                };
                both += 1;
                if a != b {
                    out.push((*w, a.clone(), b.clone()));
                }
            }
            (both, out)
        }

        /// What kinds of difference a set of them shows, with a few of them.
        fn kinds(diffs: &[(Pos, Look, Look)]) -> String {
            let n =
                |f: &dyn Fn(&Look, &Look) -> bool| diffs.iter().filter(|(_, a, b)| f(a, b)).count();
            let some: Vec<String> = diffs
                .iter()
                .take(3)
                .map(|(w, a, b)| format!("{w:?} outside {a:?} inside {b:?}"))
                .collect();
            format!(
                "{} cells (glyph {}, fg {}, bg {}, modifier {}): {}",
                diffs.len(),
                n(&|a, b| a.0 != b.0),
                n(&|a, b| a.1 != b.1),
                n(&|a, b| a.2 != b.2),
                n(&|a, b| a.3 != b.3),
                some.join("; ")
            )
        }

        /// The views from outside and inside compared, plainly and in the light view, each
        /// difference (or too few cells to tell) added to `failed` under `what`.
        fn compare(
            what: &str,
            outside: &View,
            inside: &View,
            aside: &BTreeSet<Pos>,
            failed: &mut Vec<String>,
        ) {
            for (how, a, b) in [
                ("", &outside.plain, &inside.plain),
                (" (light view)", &outside.light, &inside.light),
            ] {
                let (both, diffs) = differences(a, b, aside);
                if both < 200 {
                    failed.push(format!("{what}{how}: only {both} cells in both views"));
                } else if !diffs.is_empty() {
                    failed.push(format!("{what}{how}: {}", kinds(&diffs)));
                }
            }
        }

        /// `city`'s holo ads on the street: each ad board's node and building, and its
        /// tile.
        fn ad_boards(city: &citygen::City) -> Vec<(citygen::NodeId, citygen::BuildingId, Pos)> {
            city.net
                .nodes
                .iter()
                .filter(|n| n.kind == citygen::NodeKind::AdBoard)
                .filter(|n| n.anchor.0 == StratumKind::Street)
                .filter_map(|n| Some((n.id, n.building?, n.anchor.1)))
                .collect()
        }

        #[test]
        fn a_city_left_looks_the_same_from_outside_as_inside() {
            use crate::holds::{self, LIGHTS_OUT_MINUTES, Line};
            // the next city of the world with a holo ad nearest open ground beside it
            let mut st = run_at(22 * 60);
            let (s, at, inside) = {
                let o = st.outside.as_ref().unwrap();
                o.others()
                    .into_iter()
                    .filter(|(s, _)| s.town.is_none())
                    .filter_map(|(s, _)| {
                        let city = o.held_city(s)?;
                        let ads: Vec<Pos> = ad_boards(city).into_iter().map(|a| a.2).collect();
                        let (at, inside) = beside(&st, s, city, &ads)?;
                        let mine = o.origin(city);
                        let q = Pos::new(at.x - mine.x, at.y - mine.y);
                        let d = ads.iter().map(|a| a.chebyshev(q)).min()?;
                        Some((d, s, at, inside))
                    })
                    .min_by_key(|(d, ..)| *d)
                    .map(|(_, s, at, inside)| (s, at, inside))
                    .expect("a city with holo ads")
            };
            let home = st.outside.as_ref().unwrap().home();
            // in it: every district's lamps cut, every holo ad hijacked
            crate::outside::enter(&mut st, s).unwrap();
            let now = st.net.now(&st);
            let city = st.city.clone().unwrap();
            for d in &city.districts {
                st.net.lights_out.insert(d.id, now + LIGHTS_OUT_MINUTES);
            }
            for (node, b, _) in ad_boards(&city) {
                holds::take(
                    &mut st,
                    citygen::NetNode::City(node),
                    crate::owners::Owner::Building(b),
                    now,
                );
                holds::hijack(&mut st, node, Line::Tag);
            }
            holds::tick(&mut st);
            assert!(!holds::hijacked_ads(&st).is_empty(), "its ads hijacked");
            let keep = owned_by(&st, s, &city);
            let mut failed = Vec::new();
            // left for home: from outside, then back in; again half an hour on, its lamps
            // lit again, its ads still hijacked
            for (later, when) in [(0, "just left"), (30, "half an hour on")] {
                crate::outside::enter(&mut st, home).unwrap();
                if later > 0 {
                    crate::clock::run(
                        &mut st,
                        Some(later * sim::TICKS_PER_MINUTE),
                        crate::clock::PATHS_PER_FRAME,
                    );
                }
                let dark = |st: &AppState| {
                    let city = st.outside.as_ref().unwrap().held_city(s).unwrap();
                    !city.stratum(StratumKind::Street).unwrap().dark.is_empty()
                };
                assert_eq!(
                    dark(&st),
                    later == 0,
                    "{when}: its lamps cut till they come on"
                );
                let held = drawn(&mut st, Some(inside), at, &keep);
                assert!(
                    held.plain
                        .values()
                        .any(|l| l.1 == crate::ui::chrome::MAGENTA),
                    "{when}: a hijacked ad in sight from outside"
                );
                let mut aside = people(&st);
                crate::outside::enter(&mut st, s).unwrap();
                let live = drawn(&mut st, None, at, &keep);
                aside.extend(people(&st));
                compare(when, &held, &live, &aside, &mut failed);
            }
            assert!(
                failed.is_empty(),
                "{} differ:\n{}",
                failed.len(),
                failed.join("\n")
            );
        }

        #[test]
        fn a_city_looks_the_same_from_outside_and_inside() {
            let mut failed = Vec::new();
            let others: Vec<Settlement> = {
                let st = run_at(0);
                let o = st.outside.as_ref().unwrap();
                o.others().into_iter().map(|(s, _)| s).collect()
            };
            for (minute, hour) in HOURS {
                // every other city and town: held beside the home city, then live, from
                // the same cell, having looked round from inside it first
                for &s in &others {
                    let mut st = run_at(minute);
                    let (name, city) = {
                        let o = st.outside.as_ref().unwrap();
                        (o.name(s), o.held_city(s).unwrap().clone())
                    };
                    let (at, inside) =
                        beside(&st, s, &city, &built(&city)).expect("open ground beside it");
                    let keep = owned_by(&st, s, &city);
                    let held = drawn(&mut st, Some(inside), at, &keep);
                    let mut aside = people(&st);
                    crate::outside::enter(&mut st, s).unwrap();
                    assert_eq!(st.minute_of_day(), minute, "the hour carries over");
                    let live = drawn(&mut st, None, at, &keep);
                    aside.extend(people(&st));
                    compare(&format!("{hour} {name}"), &held, &live, &aside, &mut failed);
                }
                // the home city: live, then set aside while another is
                let mut st = run_at(minute);
                let (home, city, next) = {
                    let o = st.outside.as_ref().unwrap();
                    let next = others.iter().copied().find(|s| s.town.is_none()).unwrap();
                    (o.home(), st.city.clone().unwrap(), next)
                };
                let (at, inside) =
                    beside(&st, home, &city, &built(&city)).expect("open ground beside home");
                let keep = owned_by(&st, home, &city);
                let live = drawn(&mut st, Some(inside), at, &keep);
                let mut aside = people(&st);
                crate::outside::enter(&mut st, next).unwrap();
                let held = drawn(&mut st, None, at, &keep);
                aside.extend(people(&st));
                compare(
                    &format!("{hour} the home city set aside"),
                    &held,
                    &live,
                    &aside,
                    &mut failed,
                );
            }
            assert!(
                failed.is_empty(),
                "{} differ:\n{}",
                failed.len(),
                failed.join("\n")
            );
        }
    }
}
