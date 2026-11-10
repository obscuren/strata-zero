//! Venues in play: the Tech a crawl's grille wants, the grilles on a floor and the crawls
//! in from the city, what a lock's number names, what a step onto a venue's door or grille
//! meets, which doors in from the city stay shut to you, how far a walk across a level
//! gets past what the player can open now, and a mission's guards at their posts.

use crate::app::AppState;
use crate::credentials::{self, How};
use crate::foes::{Foe, Post};
use citygen::venue::{LockKind, MissionLayer, VenueFrame, VenueLayout, VenuePack, WayInKind};
use citygen::{BuildingId, DoorLock, FloorPlan, LinkKind, Place, Pos, StratumKind, Terrain};
use std::borrow::Cow;
use std::collections::VecDeque;

/// The Tech a crawl's grille wants.
pub fn crawl_tech() -> u8 {
    VenuePack::builtin().crawl_tech
}

/// Whether the player can crawl: Tech at the grille's mark or more.
pub fn can_crawl(st: &AppState) -> bool {
    st.character.attribute(&st.content, game::Attribute::Tech) >= crawl_tech()
}

/// What a grille says to a player who can't crawl.
pub fn screwed_shut() -> String {
    format!("a grille, screwed shut · it takes Tech {}", crawl_tech())
}

/// The crawl in whose grille is `p` on `level`, by index in the plan's crawls.
pub fn crawl_way_at(plan: &FloorPlan, level: i32, p: Pos) -> Option<usize> {
    plan.crawls
        .iter()
        .position(|c| c.level == level && c.at == p)
}

/// Whether `p` on `level` is a grille: a crawl's landing, or a crawl in's. A ladder is
/// none.
pub fn grille_at(plan: &FloorPlan, level: i32, p: Pos) -> bool {
    plan.link_at(level, p)
        .is_some_and(|l| plan.links[l].kind == LinkKind::Crawl)
        || crawl_way_at(plan, level, p).is_some()
}

/// The closed buildings whose footprint holds or touches the city tile `p`, and whether a
/// crawl may come in from `p` on `k` to each: a building the session has planned may, and
/// an unplanned one only when a venue type builds its kind and its frame has a crawl there.
fn crawl_candidates(
    st: &AppState,
    k: StratumKind,
    p: Pos,
) -> impl Iterator<Item = (BuildingId, bool)> + '_ {
    let city = st.city.as_ref();
    let pack = VenuePack::builtin();
    (city.into_iter())
        .flat_map(|c| c.buildings.iter())
        .filter(move |b| b.kind.is_closed() && b.footprint.grow(1).contains(p))
        .filter_map(move |b| {
            if st.interiors.plan_of(b.id).is_some() {
                return Some((b.id, true));
            }
            pack.for_kind(b.kind)?;
            let frame = VenueFrame::of_building(city?, b.id)?;
            (frame.crawls.iter())
                .any(|c| c.stratum == k && c.outside == p)
                .then_some((b.id, false))
        })
}

/// The crawl in from the city tile `p` on stratum `k`: the closed building whose footprint
/// holds or touches the tile and has one there, and which of its plan's crawls it is. A
/// building the session hasn't planned is planned for it only when a venue type builds
/// its kind and its frame has a crawl on that tile.
pub fn crawl_in_at(st: &AppState, k: StratumKind, p: Pos) -> Option<(BuildingId, usize)> {
    crawl_candidates(st, k, p).find_map(|(b, _)| {
        let plan = crate::interiors::plan_now(st, b)?;
        let way = (plan.crawls.iter()).position(|c| c.stratum == k && c.outside == p)?;
        Some((b, way))
    })
}

/// Keeps the plans of the venues about the city tile you stand on: one whose crawl comes
/// in from it, and one with a door beside it. Space and Look there borrow them instead of
/// laying the venue out every frame.
pub fn keep_plans_here(st: &mut AppState) {
    let Some((Place::Stratum(k), p)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    let mut unplanned: Vec<BuildingId> = (crawl_candidates(st, k, p))
        .filter(|(_, planned)| !planned)
        .map(|(b, _)| b)
        .collect();
    let Some(city) = st.city.as_ref() else {
        return;
    };
    for n in p.neighbors4() {
        let Some(b) = (city.tile(k, n))
            .filter(|t| t.terrain == Terrain::Door)
            .and_then(|t| t.building)
        else {
            continue;
        };
        let venue =
            (city.building_opt(b)).is_some_and(|x| VenuePack::builtin().for_kind(x.kind).is_some());
        if venue && st.interiors.plan_of(b).is_none() && !unplanned.contains(&b) {
            unplanned.push(b);
        }
    }
    for b in unplanned {
        st.interiors.plan(city, b);
    }
}

/// Building `b`'s plan when a venue type builds its kind: the session's, or laid out fresh.
fn venue_plan(st: &AppState, b: BuildingId) -> Option<Cow<'_, FloorPlan>> {
    let kind = st.city.as_ref()?.building_opt(b)?.kind;
    VenuePack::builtin().for_kind(kind)?;
    crate::interiors::plan_now(st, b)
}

/// The lock on `plan`'s way in through the city-map door `door` on stratum `k`, and the
/// level it stands on.
fn way_in_of(plan: &FloorPlan, k: StratumKind, door: Pos) -> Option<(i32, DoorLock)> {
    let v = plan.venue.as_deref()?;
    v.ways_in.iter().find_map(|w| match w.kind {
        WayInKind::Door { stratum, door: d } if stratum == k && d == door => {
            Some((w.level, w.lock?))
        }
        _ => None,
    })
}

/// The lock on the way in through the city-map door `door` on stratum `k` into building
/// `b`, and the level it stands on; a building of a kind no venue type builds has none.
pub fn way_in_lock(
    st: &AppState,
    b: BuildingId,
    k: StratumKind,
    door: Pos,
) -> Option<(i32, DoorLock)> {
    let plan = venue_plan(st, b)?;
    way_in_of(&plan, k, door)
}

/// The locked way in through the city-map door on tile `p` of stratum `k`: its building,
/// the level its lock stands on, the lock, and what the door is called ("staff door").
pub fn locked_door_in(
    st: &AppState,
    k: StratumKind,
    p: Pos,
) -> Option<(BuildingId, i32, DoorLock, String)> {
    let city = st.city.as_ref()?;
    let b = (city.tile(k, p))
        .filter(|t| t.terrain == Terrain::Door)?
        .building?;
    if !city.portals(b).contains(&(k, p)) {
        return None;
    }
    let plan = venue_plan(st, b)?;
    let (level, lock) = way_in_of(&plan, k, p)?;
    Some((b, level, lock, lock_label(&plan, &lock)))
}

/// Whether the city-map door `door` on `k` into building `b` stays shut to you from the
/// city: a way in whose lock doesn't open for you now.
pub fn door_shut(st: &AppState, b: BuildingId, k: StratumKind, door: Pos) -> bool {
    way_in_lock(st, b, k, door).is_some_and(|(level, lock)| {
        credentials::opens_lock(st, b, level, &lock, lock.at).is_none()
    })
}

/// A zone's name in a venue's plan: "staff". A plan with no layout has no zones.
pub fn zone_name(plan: &FloorPlan, zone: u8) -> String {
    plan.venue
        .as_deref()
        .and_then(|v| v.zones.get(zone as usize))
        .cloned()
        .unwrap_or_else(|| "zone".to_string())
}

/// What a lock's door is called: "staff door".
pub fn lock_label(plan: &FloorPlan, lock: &DoorLock) -> String {
    format!("{} door", zone_name(plan, lock.zone))
}

/// The lock numbered `number` on `level` of a venue: on a door between its rooms (always
/// locked, or one a mission can fit), or on a way in.
pub fn lock_numbered(v: &VenueLayout, level: i32, number: u16) -> Option<DoorLock> {
    let x = v.level(level)?;
    let doors = x.doors.iter().flat_map(|d| d.lock.into_iter().chain(d.fit));
    let ways = (v.ways_in.iter())
        .filter(|w| w.level == level)
        .filter_map(|w| w.lock);
    doors.chain(ways).find(|l| l.number == number)
}

/// What the lock numbered `number` on `level` of building `b` is called, as the session
/// has the building's plan: "staff door". None when no lock has that number.
pub fn lock_name(st: &AppState, b: BuildingId, level: i32, number: u16) -> Option<String> {
    let plan = st.interiors.plan_of(b)?;
    let lock = lock_numbered(plan.venue.as_deref()?, level, number)?;
    Some(lock_label(plan, &lock))
}

/// The zone card a ride on link `link` from `from` to `to` asks for: the deeper of the two
/// stops' cards; 0 asks none.
pub fn ride_card(plan: &FloorPlan, link: usize, from: i32, to: i32) -> u8 {
    let card = |level| {
        (plan.links.get(link))
            .and_then(|l| l.stop(level))
            .map_or(0, |s| s.card)
    };
    card(from).max(card(to))
}

/// What a step from `here` onto `target` on a venue's floor meets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meets {
    /// Nothing of a venue's: the step goes on as on any floor.
    Nothing,
    /// A lock or a grille that lets you through, and what the log says of it.
    Through(Option<String>),
    /// A lock or a grille that holds: the warning.
    Held(String),
}

/// What `lock` on `level` of `plan`'s building meets coming from tile `from`: it asks
/// `credentials::opens_lock`, and says how it opened or that it holds.
pub fn meets_lock(
    st: &AppState,
    plan: &FloorPlan,
    level: i32,
    lock: &DoorLock,
    from: Pos,
) -> Meets {
    let label = lock_label(plan, lock);
    match credentials::opens_lock(st, plan.building, level, lock, from) {
        None => Meets::Held(match lock.kind {
            LockKind::Keypad => format!("locked · {label}, keypad"),
            _ => format!("locked · {label}"),
        }),
        Some(How::Keycard) => Meets::Through(Some(format!("{label} · the keycard opens it"))),
        Some(How::Code(c)) => Meets::Through(Some(format!("{label} · you key in {c:04}"))),
        Some(How::Cracked | How::Free) => Meets::Through(None),
    }
}

/// What a step from `here` onto `target` on `level` of building `b` meets: a lock's door
/// asks `credentials::opens_lock` from `here`, and a grille wants a crawler.
pub fn step_onto(st: &AppState, b: BuildingId, level: i32, here: Pos, target: Pos) -> Meets {
    let Some((plan, floor)) = st.interiors.floor(b, level) else {
        return Meets::Nothing;
    };
    if let Some(lock) = floor.lock_at(target) {
        return meets_lock(st, plan, level, lock, here);
    }
    if grille_at(plan, level, target) {
        return if can_crawl(st) {
            Meets::Through(None)
        } else {
            Meets::Held(screwed_shut())
        };
    }
    Meets::Nothing
}

/// The room of `level` holding tile `p`, by index in the level's planned rooms.
pub fn room_of(plan: &FloorPlan, level: i32, p: Pos) -> Option<usize> {
    let x = plan.venue.as_deref()?.level(level)?;
    x.rooms.iter().position(|r| r.rect.contains(p))
}

/// The rooms of `level` a walk from tile `from` reaches past what the player can open now,
/// by index in the level's planned rooms: through open joins and plain doors, through a
/// lock that opens from the side the walk comes from, and into a private room that opens
/// for them. None for a plan with no layout, where every walk gets through.
pub fn reached(st: &AppState, plan: &FloorPlan, level: i32, from: Pos) -> Option<Vec<bool>> {
    let x = plan.venue.as_deref()?.level(level)?;
    let b = plan.building;
    let mut seen = vec![false; x.rooms.len()];
    let Some(start) = room_of(plan, level, from) else {
        return Some(seen);
    };
    seen[start] = true;
    let mut queue = VecDeque::from([start]);
    while let Some(r) = queue.pop_front() {
        for d in &x.doors {
            let other = match d.rooms {
                (a, o) if a == r => o,
                (o, a) if a == r => o,
                _ => continue,
            };
            if seen[other] {
                continue;
            }
            let opens = d.open
                || match d.lock {
                    Some(l) => {
                        let side = (l.free_from)
                            .filter(|p| x.rooms[r].rect.contains(*p))
                            .unwrap_or(l.at);
                        credentials::opens_lock(st, b, level, &l, side).is_some()
                    }
                    None => {
                        let to = &x.rooms[other];
                        !to.private || credentials::opens(st, b, level, to.number).is_some()
                    }
                };
            if opens {
                seen[other] = true;
                queue.push_back(other);
            }
        }
    }
    Some(seen)
}

/// The layer's guards as inert foes at their posts: name "guard", glyph '@', post set.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "a mission's copy puts its guards on its floors")
)]
pub fn guard_foes(layer: &MissionLayer, b: BuildingId) -> Vec<Foe> {
    (layer.guards.iter())
        .map(|g| {
            let place = Place::Floor {
                building: b,
                level: g.level,
            };
            Foe {
                name: "guard".to_string(),
                inert: true,
                glyph: '@',
                post: Some(Post {
                    building: b,
                    zone: g.zone,
                    patrol: g.patrol.clone(),
                }),
                ..Foe::goon(place, g.at)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
