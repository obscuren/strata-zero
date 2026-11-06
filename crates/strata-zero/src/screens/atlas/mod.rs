//! The deck's MAP: the city in half-block pixels with its listings, the places pane,
//! the keys, and what the map keeps between frames. Spec
//! `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md`, 7.

mod keys;
pub mod map;
mod pane;
mod world;

pub use keys::key;
#[cfg(test)]
pub(crate) use world::places as world_places;

use crate::app::AppState;
use crate::render::map::MapRef;
use crate::track::listings::{self, Kind, Listing, walk_from};
use crate::track::route::{self, Leg};
use crate::track::{ListingId, Pulses, Source, Target, target_now};
use crate::ui::chrome::{CYAN, FAINTER, PANEL_BG};
use citygen::{Grid, Place, Pos, StratumKind};
use map::MapLayout;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::borrow::Cow;

/// The MAP's hint row, the pane open and folded (spec 7).
const HINT_OPEN: &str =
    "←↑↓→ explore · [ ] places · < > kinds · t track · +− zoom · f fold · tab close";
const HINT_FOLDED: &str =
    "←↑↓→ explore · [ ] places · < > kinds · t track · +− zoom · f unfold · tab close";

/// What the map remembers between frames; lasts the session, reset with the city.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AtlasState {
    /// The stratum the cursor belongs to; another one starts it again.
    pub stratum: Option<StratumKind>,
    /// In tiles; none until the map first draws.
    pub cursor: Option<Pos>,
    /// The tile at the map's top-left cell.
    pub origin: Pos,
    pub zoom: Zoom,
    /// The kind shown; none is `all`.
    pub kind: Option<Kind>,
    pub folded: bool,
    /// The places list's first row in view.
    pub scroll: usize,
    /// The listing `[ ]` stepped to and the tile they put the cursor on: it stays the
    /// one under the cursor while the cursor stays there, so listings that share a
    /// cell each get their turn.
    pub picked: Option<(ListingId, Kind, Pos)>,
    /// The next layout centres the view on the cursor (after a zoom).
    pub recentre: bool,
    /// Where the map was last laid out, for the keys.
    pub layout: Option<MapLayout>,
    cache: Cache,
    /// The pulses on the walk from your building's way out, while you are inside one
    /// (on the street the map's walk is the track's leg, with the track's pulses).
    pulses: Pulses,
    /// Zoom 4's cursor and kind.
    pub world: world::WorldView,
}

impl AtlasState {
    /// Keeps what the last draw worked out: the map's stratum, the cursor and the view.
    pub fn settle(&mut self, layout: &MapLayout) {
        self.stratum = layout.stratum;
        self.cursor = layout.cursor;
        self.origin = layout.origin;
        self.recentre = false;
        self.layout = Some(*layout);
    }

    /// Leaving the MAP tab: the next visit starts the cursor again (on the track, else
    /// on you).
    pub fn leave(&mut self) {
        self.cursor = None;
        self.picked = None;
    }
}

/// How close the map is: zoom 3 the whole stratum, 2 and 1 closer, 4 the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Zoom {
    One,
    #[default]
    Two,
    Fit,
    /// Zoom 4: the world (`world`).
    World,
}

impl Zoom {
    /// How close the zoom draws when `fit` tiles a pixel fit the whole stratum: the
    /// tiles a pixel covers and the pixels a tile spans, across and down (one of them
    /// 1). Fit is the whole stratum; 2 and 1 are two tiles a pixel and one where fit is
    /// more than both, else each a step closer: from a tile a pixel, a tile over 2
    /// pixels, then 4. So each zoom in is closer, in a city of any size.
    pub fn scale(self, fit: i32) -> (i32, i32) {
        let fit = (fit.max(1), 1);
        let closer = |(s, m): (i32, i32)| if s > 1 { (s - 1, 1) } else { (1, 2 * m) };
        let two = if fit.0 > 2 { (2, 1) } else { closer(fit) };
        match self {
            Zoom::One if fit.0 > 2 => (1, 1),
            Zoom::One => closer(two),
            Zoom::Two => two,
            Zoom::Fit | Zoom::World => fit,
        }
    }

    /// The zoom's number on the title row: 1, 2, 3 the whole stratum, 4 the world.
    pub fn number(self) -> u8 {
        match self {
            Zoom::One => 1,
            Zoom::Two => 2,
            Zoom::Fit => 3,
            Zoom::World => 4,
        }
    }

    /// One step in, `+`: fit, 2, 1.
    pub fn closer(self) -> Zoom {
        match self {
            Zoom::World => Zoom::Fit,
            Zoom::Fit => Zoom::Two,
            _ => Zoom::One,
        }
    }

    /// One step out, `-`: 1, 2, fit.
    pub fn further(self) -> Zoom {
        match self {
            Zoom::One => Zoom::Two,
            _ => Zoom::Fit,
        }
    }
}

/// The listings and the walk the map draws, and what they were made from.
#[derive(Clone, Debug, Default)]
struct Cache {
    key: Option<Key>,
    listings: Vec<Listing>,
    /// Steps of walk from you to each tile of the map, for the spot under the cursor.
    field: Option<Grid<u32>>,
    /// The walk on the map from your building's way out, while you are inside it.
    walk: Option<Leg>,
}

/// A cache never tells two map states apart.
impl PartialEq for Cache {
    fn eq(&self, _: &Cache) -> bool {
        true
    }
}

/// What the cache was made from: the map, where the walk starts, where you are, the
/// target, and the second (for jobs, the story and the clinics' hours).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    stratum: StratumKind,
    from: Option<Pos>,
    place: Option<Place>,
    target: Option<Target>,
    second: u64,
}

impl Key {
    fn now(st: &AppState) -> Key {
        let stratum = map_stratum(st);
        Key {
            stratum,
            from: walk_from(st, stratum),
            place: st.player.as_ref().map(|p| p.place),
            target: target_now(st),
            second: st.tick / 30,
        }
    }
}

impl Cache {
    fn new(st: &AppState, key: Key) -> Cache {
        Cache {
            key: Some(key),
            listings: listings::listings(st, key.stratum),
            field: listings::walk_field(st, key.stratum),
            walk: way_out_walk(st, key.stratum),
        }
    }
}

/// The cache when it is fresh, else one made now.
fn cached(st: &AppState) -> Cow<'_, Cache> {
    let key = Key::now(st);
    if st.atlas.cache.key == Some(key) {
        Cow::Borrowed(&st.atlas.cache)
    } else {
        Cow::Owned(Cache::new(st, key))
    }
}

/// Brings the map's listings and walk up to date, and the pulses on that walk; the deck
/// calls it each frame the map shows, so a draw finds them made.
pub fn refresh(st: &mut AppState) {
    let key = Key::now(st);
    if st.atlas.cache.key != Some(key) {
        st.atlas.cache = Cache::new(st, key);
    }
    let walk = st
        .atlas
        .cache
        .walk
        .as_ref()
        .map(|w| (w.place, w.path.len()));
    let now = st.tick;
    st.atlas.pulses.step(walk, now);
}

/// The listings of the map's stratum, nearest first: the cached ones while fresh.
pub fn listed(st: &AppState) -> Cow<'_, [Listing]> {
    match cached(st) {
        Cow::Borrowed(c) => Cow::Borrowed(c.listings.as_slice()),
        Cow::Owned(c) => Cow::Owned(c.listings),
    }
}

/// The stratum the map shows: yours; the Street while you are inside a building, or
/// your body's while you are jacked in.
pub fn map_stratum(st: &AppState) -> StratumKind {
    let on = |place: Place| match place {
        Place::Stratum(k) if k != StratumKind::Net => Some(k),
        _ => None,
    };
    st.player
        .as_ref()
        .and_then(|p| on(p.place).or_else(|| p.body.and_then(|(place, _)| on(place))))
        .unwrap_or(StratumKind::Street)
}

/// The places pane's width, its border included: 36 cells at 110 columns and wider,
/// else 30.
pub fn pane_width(body: Rect) -> u16 {
    if body.width + 2 >= 110 { 36 } else { 30 }
}

/// The map's cells in the deck's body: under the title row, left of the pane (and a
/// spare column), or of the strip when the pane is folded.
fn map_area(body: Rect, folded: bool) -> Rect {
    let side = if folded {
        pane::STRIP
    } else {
        pane_width(body) + 1
    };
    Rect::new(
        body.x,
        body.y.saturating_add(1),
        body.width.saturating_sub(side),
        body.height.saturating_sub(1),
    )
}

/// The map as it lays out in the deck's `body`, the pane open or folded.
fn layout_in(st: &AppState, body: Rect) -> MapLayout {
    let folded = st.atlas.folded;
    MapLayout {
        pane: if folded { 0 } else { pane_width(body) },
        ..map::layout(st, map_area(body, folded))
    }
}

/// The map laid out again where `last` was, as it would draw now.
fn relayout(st: &AppState, last: &MapLayout) -> MapLayout {
    MapLayout {
        pane: last.pane,
        ..map::layout(st, last.area)
    }
}

/// Draws the MAP tab into the deck's `body`: the title row, the map, the places pane or,
/// folded, the strip, and the card of the listing under the cursor (spec 7). The deck
/// settles what it returns.
pub fn draw(st: &AppState, body: Rect, buf: &mut Buffer) -> MapLayout {
    let body = body.intersection(buf.area);
    let ground = Style::default().bg(PANEL_BG);
    for y in body.y..body.bottom() {
        for x in body.x..body.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_style(ground);
        }
    }
    if st.atlas.zoom == Zoom::World && world::available(st) {
        return world::draw(st, body, buf);
    }
    let l = layout_in(st, body);
    if l.stratum.is_none() || body.height < 2 {
        map::draw_at(st, &l, &cached(st), None, buf);
        return l;
    }
    let cache = cached(st);
    let all = &cache.listings;
    let card = under(st, &l, all).and_then(|i| {
        let at = pane::card_box(&l)?;
        Some((at, &all[i]))
    });
    map::draw_at(st, &l, &cache, card.map(|(at, _)| at), buf);
    title(st, &l, body, buf);
    if st.atlas.folded {
        pane::strip(st, body, all, buf);
    } else {
        pane::pane(st, &l, body, &cache, buf);
    }
    if let Some((at, x)) = card {
        pane::card(st, &l, at, x, buf);
    }
    l
}

/// The title row: the city and the map's stratum, then the zoom and the key to the
/// colours, cut at the map's edge.
fn title(st: &AppState, l: &MapLayout, body: Rect, buf: &mut Buffer) {
    let (Some(city), Some(k)) = (st.city.as_ref(), l.stratum) else {
        return;
    };
    let head = format!(
        "{} · {}",
        city.name.to_uppercase(),
        k.label().to_uppercase()
    );
    // the zoom's number, the same in every city (the city's map at the world's zoom,
    // with no world to show, is the whole city's)
    let zoom = match st.atlas.zoom {
        Zoom::World => Zoom::Fit,
        z => z,
    };
    let rest = format!(" · zoom {}", zoom.number());
    let right = l.area.right();
    let x = body.x + 1;
    let bold = Style::default()
        .fg(CYAN)
        .bg(PANEL_BG)
        .add_modifier(Modifier::BOLD);
    pane::put(buf, x, body.y, &head, bold, right);
    let x = x.saturating_add(head.chars().count() as u16);
    pane::put(buf, x, body.y, &rest, Style::default().fg(FAINTER), right);
}

/// The hint row: the keys, and `f fold` or `f unfold`.
pub fn hint(st: &AppState) -> &'static str {
    if st.atlas.zoom == Zoom::World && world::available(st) {
        return world::HINT;
    }
    if st.atlas.folded {
        HINT_FOLDED
    } else {
        HINT_OPEN
    }
}

/// The listing under the cursor: the one `[ ]` stepped to, while the cursor stays on
/// it; else, of the listings shown, the one whose marker shows on the cursor's cell;
/// else the one whose building the cursor is on.
pub fn under_cursor(st: &AppState, layout: &MapLayout) -> Option<Listing> {
    let all = listed(st);
    under(st, layout, &all).map(|i| all[i].clone())
}

/// `under_cursor` as an index into `all`.
fn under(st: &AppState, l: &MapLayout, all: &[Listing]) -> Option<usize> {
    let cursor = l.cursor?;
    let shown = |x: &Listing| map::shows(st, x);
    if let Some((id, kind, at)) = st.atlas.picked
        && at == cursor
        && let Some(i) = all
            .iter()
            .position(|x| x.id == id && x.kind == kind && shown(x))
    {
        return Some(i);
    }
    let best = |on: &dyn Fn(&Listing) -> bool| {
        all.iter()
            .enumerate()
            .filter(|(_, x)| shown(x) && on(x))
            .min_by_key(|(i, x)| (map::rank(st, x), *i))
            .map(|(i, _)| i)
    };
    let cell = l.cell_of(cursor)?;
    best(&|x| l.cell_of(x.tile) == Some(cell)).or_else(|| {
        let city = st.city.as_ref()?;
        best(&|x| {
            x.building
                .and_then(|b| city.building_opt(b))
                .is_some_and(|b| b.footprint.contains(cursor))
        })
    })
}

/// The walkable tile nearest the middle of the tiles the cursor's cell covers, within
/// 8 tiles of it: the spot `t` tracks off the listings.
pub fn spot_under_cursor(st: &AppState, layout: &MapLayout) -> Option<Pos> {
    let (city, k) = (st.city.as_ref()?, layout.stratum?);
    let mid = layout.tiles_of(layout.cell_of(layout.cursor?)?).center();
    let ok = route::open(MapRef::stratum_of(city, k), true, &[], &[]);
    (-8..=8)
        .flat_map(|dy| (-8..=8).map(move |dx| mid.offset(dx, dy)))
        .filter(|p| ok(*p))
        .min_by_key(|p| (p.dist2(mid), p.y, p.x))
}

/// What `t` tracks off the listings: the spot under the cursor, arrived at within the
/// tiles a pixel covers.
pub(super) fn spot_source(st: &AppState, layout: &MapLayout) -> Option<Source> {
    Some(Source::Spot {
        place: Place::Stratum(layout.stratum?),
        pos: spot_under_cursor(st, layout)?,
        radius: layout.s.max(1),
    })
}

/// While you are inside a building, the walk on the map from its way out to the
/// tracked target; none when the target is in there with you.
fn way_out_walk(st: &AppState, k: StratumKind) -> Option<Leg> {
    let Place::Floor { building, .. } = st.player.as_ref()?.place else {
        return None;
    };
    let target = target_now(st)?;
    let with_you = match target {
        Target::Building(b) => b == building,
        Target::Spot {
            place: Place::Floor { building: b, .. },
            ..
        } => b == building,
        _ => false,
    };
    if with_you {
        return None;
    }
    route::leg_from(st, Place::Stratum(k), walk_from(st, k)?, target).ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::screens::explore::MapScreen;

    pub(super) const STREET: StratumKind = StratumKind::Street;
    pub(super) const ON_STREET: Place = Place::Stratum(STREET);

    pub(super) fn put(st: &mut AppState, place: Place, pos: Pos) {
        let p = st.player.as_mut().unwrap();
        p.place = place;
        p.pos = pos;
    }

    /// The home city, awake, you on the street outside the megablock's door.
    pub(crate) fn outside() -> AppState {
        let mut st = state_home();
        MapScreen::walking(&mut st);
        let b = st.home.unwrap().home.building;
        let city = st.city.as_ref().unwrap();
        st.interiors.ensure(city, b, 0).unwrap();
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        let exit = floor.exits.iter().find(|e| e.stratum == STREET).unwrap();
        let landing = exit.landing;
        put(&mut st, ON_STREET, landing);
        st
    }

    pub(crate) fn you(st: &AppState) -> Pos {
        st.player.as_ref().unwrap().pos
    }

    /// The deck's body at 120 × 40 and at 100 × 30.
    pub(super) const BODY_120: Rect = Rect::new(1, 10, 118, 27);
    pub(super) const BODY_100: Rect = Rect::new(1, 5, 98, 22);

    /// One frame of the MAP tab as the deck runs it: brought up to date, drawn into
    /// `body` of a buffer the terminal's size, and settled.
    pub(super) fn frame(st: &mut AppState, body: Rect) -> Buffer {
        refresh(st);
        let mut buf = Buffer::empty(Rect::new(0, 0, body.right() + 1, body.bottom() + 3));
        let l = draw(st, body, &mut buf);
        st.atlas.settle(&l);
        buf
    }

    pub(super) fn press(st: &mut AppState, c: char) {
        let code = crossterm::event::KeyCode::Char(c);
        assert!(key(st, code), "{c} is the map's");
    }

    /// Row `y` of the buffer over the columns `xs`.
    pub(super) fn text(buf: &Buffer, y: u16, xs: std::ops::Range<u16>) -> String {
        xs.map(|x| buf[(x, y)].symbol().to_string()).collect()
    }

    pub(super) fn laid(st: &AppState) -> MapLayout {
        st.atlas.layout.expect("drawn")
    }

    /// The nearest ripperdoc on the street that a walk reaches.
    pub(crate) fn clinic(st: &AppState) -> Listing {
        listings::listings(st, STREET)
            .into_iter()
            .find(|l| l.kind == Kind::Docs && l.walk.is_some())
            .expect("a ripperdoc on the street")
    }

    /// A street tile a few steps from you where the map lists nothing under the cursor,
    /// with a spot a walk reaches; the cursor left there and drawn.
    pub(super) fn open_ground(st: &mut AppState, body: Rect) -> Pos {
        let me = walk_from(st, STREET).expect("a walk from you");
        let size = st.city.as_ref().unwrap().size;
        let mut tiles: Vec<Pos> = size
            .rect()
            .iter()
            .filter(|p| (6..30).contains(&p.chebyshev(me)))
            .collect();
        tiles.sort_by_key(|p| (p.manhattan(me), p.y, p.x));
        for p in tiles {
            st.atlas.stratum = Some(STREET);
            st.atlas.cursor = Some(p);
            let l = layout_in(st, body);
            let spot = spot_under_cursor(st, &l);
            let walked = spot.is_some_and(|q| {
                let f = listings::walk_field(st, STREET).unwrap();
                f.get(q).is_some_and(|n| *n != u32::MAX && *n > 4)
            });
            if under_cursor(st, &l).is_none() && walked {
                frame(st, body);
                return p;
            }
        }
        panic!("no open ground near you")
    }

    #[test]
    fn the_title_names_the_map() {
        let mut st = outside();
        let buf = frame(&mut st, BODY_120);
        let city = st.city.as_ref().unwrap().name.to_uppercase();
        let head = format!("{city} · STREET");
        let row = text(&buf, BODY_120.y, 2..BODY_120.right());
        let want = format!("{head} · zoom 2 ");
        assert!(row.starts_with(&want), "{row}");
        assert!(!row.contains("remembered"), "no fog to explain: {row}");
        let at = |x: u16| &buf[(x, BODY_120.y)];
        assert_eq!(at(2).fg, CYAN);
        assert!(at(2).modifier.contains(Modifier::BOLD));
        let rest = 2 + head.chars().count() as u16 + 1;
        assert_eq!(at(rest).fg, FAINTER);
        // Cut at the map's edge, short of the pane.
        let buf = frame(&mut st, Rect::new(1, 5, 60, 22));
        let l = laid(&st);
        assert!(text(&buf, 5, 1..l.area.right()).contains(&head));
        assert_eq!(buf[(l.area.right(), 5)].symbol(), " ");
    }

    #[test]
    fn the_listings_are_cached_until_their_inputs_change() {
        let mut st = outside();
        assert!(matches!(listed(&st), Cow::Owned(_)), "nothing cached yet");
        refresh(&mut st);
        assert!(matches!(listed(&st), Cow::Borrowed(_)));
        assert_eq!(listed(&st).to_vec(), listings::listings(&st, STREET));
        st.tick += 30;
        assert!(matches!(listed(&st), Cow::Owned(_)), "a second on");
        refresh(&mut st);
        let here = you(&st);
        put(&mut st, ON_STREET, here.offset(0, 1));
        assert!(matches!(listed(&st), Cow::Owned(_)), "you moved");
    }

    #[test]
    fn inside_the_maps_walk_has_pulses_of_its_own() {
        let mut st = outside();
        let landing = you(&st);
        let doc = clinic(&st);
        crate::track::set(&mut st, crate::track::Source::Listing(doc.id)).unwrap();
        let b = st.home.unwrap().home.building;
        let lift = st.interiors.floor(b, 0).unwrap().0.lift.unwrap();
        let lobby = Place::Floor {
            building: b,
            level: 0,
        };
        put(&mut st, lobby, lift);
        st.tick = 300;
        crate::track::tick(&mut st);
        refresh(&mut st);
        let walk = st
            .atlas
            .cache
            .walk
            .clone()
            .expect("the walk from the way out");
        let first = crate::track::Pulse {
            t0: 300,
            len: walk.path.len(),
        };
        assert_eq!(st.atlas.pulses.all(), [first]);
        st.tick = 310;
        refresh(&mut st);
        assert_eq!(st.atlas.pulses.all(), [first]);
        st.tick = 389;
        refresh(&mut st);
        assert!(st.atlas.pulses.all().iter().all(|p| p.t0 == 300));
        st.tick = 390;
        refresh(&mut st);
        let second = crate::track::Pulse { t0: 390, ..first };
        assert_eq!(st.atlas.pulses.all().last(), Some(&second));
        // Back on the street the map runs the track's own.
        put(&mut st, ON_STREET, landing);
        crate::track::tick(&mut st);
        refresh(&mut st);
        assert!(st.atlas.cache.walk.is_none());
        assert!(st.atlas.pulses.all().is_empty());
    }

    #[test]
    fn a_new_city_resets_the_map() {
        let mut st = outside();
        st.atlas.cursor = Some(Pos::new(3, 3));
        st.atlas.zoom = Zoom::Fit;
        let city = st.city.clone().unwrap();
        st.enter_city_at(city, 0);
        assert_eq!(st.atlas, AtlasState::default());
    }
}
