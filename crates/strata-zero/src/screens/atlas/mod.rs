//! The deck's MAP: the city in half-block pixels with its listings, and what the map
//! keeps between frames. Spec
//! `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md`, 7.

pub mod map;

use crate::app::AppState;
use crate::track::listings::{self, Kind, Listing, walk_from};
use crate::track::route::{self, Leg};
use crate::track::{Target, target_now};
use citygen::{Place, Pos, StratumKind};
use map::MapLayout;
use std::borrow::Cow;

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
    pub scroll: usize,
    cache: Cache,
}

impl AtlasState {
    /// Keeps what the last draw worked out: the map's stratum, the cursor and the view.
    pub fn settle(&mut self, layout: &MapLayout) {
        self.stratum = layout.stratum;
        self.cursor = layout.cursor;
        self.origin = layout.origin;
    }
}

/// Tiles a pixel covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Zoom {
    One,
    #[default]
    Two,
    Fit,
}

impl Zoom {
    /// The tiles a pixel covers when `fit` fits the whole stratum: never more than fit.
    pub fn tiles(self, fit: i32) -> i32 {
        let fit = fit.max(1);
        match self {
            Zoom::One => 1,
            Zoom::Two => fit.min(2),
            Zoom::Fit => fit,
        }
    }
}

/// The listings and the walk the map draws, and what they were made from.
#[derive(Clone, Debug, Default)]
struct Cache {
    key: Option<Key>,
    listings: Vec<Listing>,
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

/// Brings the map's listings and walk up to date; the deck calls it each frame the map
/// shows, so a draw finds them made.
pub fn refresh(st: &mut AppState) {
    let key = Key::now(st);
    if st.atlas.cache.key != Some(key) {
        st.atlas.cache = Cache::new(st, key);
    }
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
pub(super) mod tests {
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
    pub(super) fn outside() -> AppState {
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

    pub(super) fn you(st: &AppState) -> Pos {
        st.player.as_ref().unwrap().pos
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
    fn a_new_city_resets_the_map() {
        let mut st = outside();
        st.atlas.cursor = Some(Pos::new(3, 3));
        st.atlas.zoom = Zoom::Fit;
        let city = st.city.clone().unwrap();
        st.enter_city_at(city, 0);
        assert_eq!(st.atlas, AtlasState::default());
    }
}
