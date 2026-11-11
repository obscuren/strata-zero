//! Venues in play: the Tech a crawl's grille wants, the grilles on a floor and the crawls
//! in from the city, what a lock's number names, what a step onto a venue's door or grille
//! meets, which doors in from the city stay shut to you, how far a walk across a level
//! gets past what the player can open now, and a mission's guards at their posts. A
//! mission's copy of a venue is in `copy`, and the dev sites the console and the title
//! lay are in `dev`.

mod copy;
mod dev;

pub(crate) use copy::copy_floor;
pub use copy::{
    DoorMark, MissionCopy, begin, copy_live, door_mark_at, door_marks, end, note_at, tick,
};
pub use dev::{building_at_door, dev_site, dev_start};
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "the DEVELOPER menu's SITE NET row (Task 17) and the site's tests start them"
    )
)]
pub use dev::{dev_site_net, dev_start_with};

/// The security of the DEVELOPER menu's SITE NET start (`dev_site_net`).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the DEVELOPER menu's SITE NET row starts it from Task 17 on"
    )
)]
pub const SITE_NET_SECURITY: u8 = 190;

use crate::app::AppState;
use crate::credentials::{self, How};
use crate::foes::{Foe, Post};
use citygen::venue::{
    Keys, LockKind, MissionLayer, VenueFrame, VenueGraph, VenueLayout, VenuePack, WayInKind, Ways,
};
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

/// Building `b`'s plan as the session has it, else, when a venue type builds its kind,
/// laid out fresh.
fn venue_plan(st: &AppState, b: BuildingId) -> Option<Cow<'_, FloorPlan>> {
    if let Some(plan) = st.interiors.plan_of(b) {
        return Some(Cow::Borrowed(plan));
    }
    let kind = st.city.as_ref()?.building_opt(b)?.kind;
    VenuePack::builtin().for_kind(kind)?;
    crate::interiors::plan_now(st, b)
}

/// The lock on `plan`'s way in through the city-map door `door` on stratum `k`, and the
/// level it stands on; none on a level the plan lacks, where the door is barred.
fn way_in_of(plan: &FloorPlan, k: StratumKind, door: Pos) -> Option<(i32, DoorLock)> {
    let v = plan.venue.as_deref()?;
    v.ways_in.iter().find_map(|w| match w.kind {
        WayInKind::Door { stratum, door: d }
            if stratum == k && d == door && plan.has_level(w.level) =>
        {
            Some((w.level, w.lock?))
        }
        _ => None,
    })
}

/// Whether `plan`'s city-map door `door` on stratum `k` is barred: a venue's door onto a
/// level the plan lacks, which the layout lacks or a mission's copy doesn't keep.
fn barred_in(plan: &FloorPlan, k: StratumKind, door: Pos) -> bool {
    (plan.venue.as_deref()).is_some_and(|v| {
        (v.frame.doors.iter()).any(|d| d.stratum == k && d.door == door && !plan.has_level(d.level))
    })
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
/// city: a barred door, or a way in whose lock doesn't open for you now.
pub fn door_shut(st: &AppState, b: BuildingId, k: StratumKind, door: Pos) -> bool {
    let Some(plan) = venue_plan(st, b) else {
        return false;
    };
    barred_in(&plan, k, door)
        || way_in_of(&plan, k, door).is_some_and(|(level, lock)| {
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

/// The zone card a ride on link `link` to level `to` asks for: the card of the stop you get
/// off at, never the one you get on at, so getting out is free; 0 asks none.
pub fn ride_card(plan: &FloorPlan, link: usize, to: i32) -> u8 {
    (plan.links.get(link))
        .and_then(|l| l.stop(to))
        .map_or(0, |s| s.card)
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

/// Whether the building's alarm holds `lock` shut: one the Net could open, in a building
/// on alert, which no crack opens until the alert ends.
pub fn locked_down(st: &AppState, b: BuildingId, lock: &DoorLock) -> bool {
    lock.ways.has(Ways::NET) && st.net.alert(b, st.net.now(st))
}

/// What `lock` on `level` of `plan`'s building meets coming from tile `from`: it asks
/// `credentials::opens_lock`, and says how it opened or that it holds ("locked down" in
/// an alarm).
pub fn meets_lock(
    st: &AppState,
    plan: &FloorPlan,
    level: i32,
    lock: &DoorLock,
    from: Pos,
) -> Meets {
    let label = lock_label(plan, lock);
    match credentials::opens_lock(st, plan.building, level, lock, from) {
        None => {
            let state = if locked_down(st, plan.building, lock) {
                "locked down"
            } else {
                "locked"
            };
            Meets::Held(match lock.kind {
                LockKind::Keypad => format!("{state} · {label}, keypad"),
                _ => format!("{state} · {label}"),
            })
        }
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
    // A mission's copy fits locks to doors the plain building leaves unlocked: its drawn
    // floor holds them.
    let floor = st.interiors.floor(b, level).map(|(_, f)| f);
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
            let lock = d
                .lock
                .or_else(|| floor.and_then(|f| f.lock_at(d.at)).copied());
            let opens = d.open
                || match lock {
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

/// The work floors of `plan` where someone at work is met from the city with no key: in
/// a venue, those whose every room with a station is reached from a way in with no means
/// and no key; None for a plan with no layout, where everyone at work is.
pub fn floors_met_without_a_key(plan: &FloorPlan) -> Option<Vec<i32>> {
    let v = plan.venue.as_deref()?;
    let g = VenueGraph::of(v, &[], None);
    let reached = g.reach(&g.starts, Ways::default(), Keys::default()).nodes;
    let met = |level: i32| {
        v.level(level).is_some_and(|x| {
            (0..x.rooms.len())
                .filter(|&r| !x.rooms[r].stations.is_empty())
                .all(|r| g.room(level, r).is_some_and(|n| reached[n]))
        })
    };
    Some(plan.work_floors().into_iter().filter(|&l| met(l)).collect())
}

/// The layer's guards as inert foes at their posts: name "guard", glyph '@', post set.
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
