//! Casting: each role of the book on a person of the city (spec §5), and the street
//! tile a person works at (§6).

use super::{Pick, Role};
use crate::app::AppState;
use citygen::rng::hash64;
use citygen::{City, Pos, Rect, StratumKind};
use sim::{PersonId, Work};
use std::cmp::Reverse;
use std::collections::BTreeSet;

/// The profession `NearestFixer` casts from.
const FIXER: &str = "fixer";
/// How far past a footprint's longer side the search for its street tile reaches.
const RING_SLACK: i32 = 16;

/// Binds every role to a person, in the book's order: never one person to two roles,
/// never a dead one. A cast person takes the role's name; a role nobody can play is
/// left out of `missions.cast`.
pub fn cast(st: &mut AppState) {
    st.missions.cast.clear();
    let Some(door) = home_door(st) else {
        return;
    };
    let seed = st.city.as_ref().map_or(0, |c| c.seed);
    let roles: Vec<(String, Role)> = st
        .book
        .cast
        .iter()
        .map(|(id, r)| (id.clone(), r.clone()))
        .collect();
    for (i, (id, role)) in roles.into_iter().enumerate() {
        let taken: BTreeSet<PersonId> = st.missions.cast.values().copied().collect();
        let Some(who) = pick(st, &role.pick, door, hash64(seed, i as u64), &taken) else {
            continue;
        };
        if let Some(w) = st.world.as_mut() {
            w.rename(who, &role.name);
        }
        st.missions.cast.insert(id, who);
    }
}

/// The home building's first door, where casting measures from, as the jobs do.
pub fn home_door(st: &AppState) -> Option<Pos> {
    let b = st.home?.home.building;
    let city = st.city.as_ref()?;
    city.portals(b)
        .first()
        .map(|(_, p)| *p)
        .or_else(|| city.building_opt(b).map(|x| x.footprint.center()))
}

/// The person a pick finds, measured from `door`, `roll` choosing among equals.
fn pick(
    st: &AppState,
    pick: &Pick,
    door: Pos,
    roll: u64,
    taken: &BTreeSet<PersonId>,
) -> Option<PersonId> {
    let world = st.world.as_ref()?;
    let city = st.city.as_ref()?;
    let free = |id: PersonId| !taken.contains(&id) && !world.is_dead(id);
    match pick {
        Pick::NearestFixer => world
            .people()
            .iter()
            .filter(|p| p.profession == FIXER && free(p.id))
            .filter_map(|p| match p.work {
                Work::Spot(pos) => Some((pos.manhattan(door), p.id)),
                _ => None,
            })
            .min_by_key(|(d, id)| (*d, id.0))
            .map(|(_, id)| id),
        Pick::Worker {
            professions,
            at,
            range: (min, max),
        } => {
            let all: Vec<(i32, PersonId)> = world
                .people()
                .iter()
                .filter(|p| professions.contains(&p.profession) && free(p.id))
                .filter(|p| match p.work {
                    Work::Spot(_) => true,
                    Work::Building(b) => {
                        at.is_empty() || city.building_opt(b).is_some_and(|x| at.contains(&x.kind))
                    }
                    Work::Roaming => false,
                })
                .filter_map(|p| Some((work_tile(st, p.id)?.manhattan(door), p.id)))
                .collect();
            let within: Vec<PersonId> = all
                .iter()
                .filter(|(d, _)| (*min..=*max).contains(d))
                .map(|(_, id)| *id)
                .collect();
            if !within.is_empty() {
                return Some(within[(roll % within.len() as u64) as usize]);
            }
            all.iter()
                .filter(|(d, _)| d > max)
                .min_by_key(|(d, id)| (*d, id.0))
                .or_else(|| {
                    all.iter()
                        .filter(|(d, _)| d < min)
                        .max_by_key(|(d, id)| (*d, Reverse(id.0)))
                })
                .map(|(_, id)| *id)
        }
    }
}

/// Where a person works, on the street: a spot is its tile; a building gives the
/// walkable Street tile just outside its footprint nearest its door (closed kinds) or
/// its middle (walk-in kinds), searched ring by ring. None for a roamer.
pub fn work_tile(st: &AppState, id: PersonId) -> Option<Pos> {
    let city = st.city.as_ref()?;
    match st.world.as_ref()?.people().get(id.0 as usize)?.work {
        Work::Spot(pos) => Some(pos),
        Work::Roaming => None,
        Work::Building(b) => {
            let fp = city.building_opt(b)?.footprint;
            // A walk-in kind has no portals.
            let portals = city.portals(b);
            let from = portals
                .iter()
                .find(|(k, _)| *k == StratumKind::Street)
                .or(portals.first())
                .map_or_else(|| fp.center(), |(_, p)| *p);
            outside(city, fp, from)
        }
    }
}

/// The walkable Street tile outside `fp` nearest `from`, ring by ring.
fn outside(city: &City, fp: Rect, from: Pos) -> Option<Pos> {
    (0..=fp.w.max(fp.h) + RING_SLACK).find_map(|r| {
        ring(from, r)
            .filter(|p| !fp.contains(*p) && city.walkable(StratumKind::Street, *p))
            .min_by_key(|p| (p.manhattan(from), p.y, p.x))
    })
}

/// The tiles `r` steps from `c`, as a king walks.
fn ring(c: Pos, r: i32) -> impl Iterator<Item = Pos> {
    (-r..=r)
        .flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(move |(dx, dy)| dx.abs().max(dy.abs()) == r)
        .map(move |(dx, dy)| c.offset(dx, dy))
}

/// `{role.place}`: the work building's name, `the corner` for a spot.
pub fn place_of(st: &AppState, id: PersonId) -> String {
    let work = st
        .world
        .as_ref()
        .and_then(|w| w.people().get(id.0 as usize))
        .map(|p| p.work);
    match work {
        Some(Work::Building(b)) => crate::net::building_name(st, b),
        Some(Work::Spot(_)) => "the corner".to_string(),
        _ => "the street".to_string(),
    }
}

/// `{role.district}`: the district of the work tile.
pub fn district_of(st: &AppState, id: PersonId) -> String {
    let pos = work_tile(st, id).or_else(|| st.world.as_ref()?.pos_of(id));
    st.city
        .as_ref()
        .zip(pos)
        .and_then(|(c, pos)| c.districts.iter().find(|d| d.bounds.contains(pos)))
        .map_or_else(|| "the street".to_string(), |d| d.name.clone())
}

/// `{role.hours}`: the shift, as the jobs write it.
pub fn hours_of(st: &AppState, id: PersonId) -> String {
    st.world
        .as_ref()
        .and_then(|w| w.work_hours(id))
        .map_or_else(|| "all hours".to_string(), crate::jobs::hours_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;

    #[test]
    fn work_tile_of_a_walk_in_stall_is_a_walkable_street_tile_outside_it() {
        let st = state_home();
        let city = st.city.as_ref().unwrap();
        let (id, b) = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .find_map(|p| match p.work {
                Work::Building(b) if !city.building(b).kind.is_closed() => Some((p.id, b)),
                _ => None,
            })
            .expect("someone works a walk-in stall");
        let fp = city.building(b).footprint;
        let tile = work_tile(&st, id).expect("a work tile");
        assert!(!fp.contains(tile), "{tile:?} is inside {fp:?}");
        assert!(city.walkable(StratumKind::Street, tile), "{tile:?}");
        // The nearest such tile to the stall's middle: none closer by a ring.
        let ring = tile.chebyshev(fp.center());
        assert!(ring <= fp.w.max(fp.h), "{tile:?} is {ring} from {fp:?}");
        for y in fp.center().y - ring + 1..fp.center().y + ring {
            for x in fp.center().x - ring + 1..fp.center().x + ring {
                let p = Pos::new(x, y);
                assert!(
                    fp.contains(p) || !city.walkable(StratumKind::Street, p),
                    "{p:?} is nearer than {tile:?}"
                );
            }
        }
        // A spot worker's tile is the spot.
        let fixer = crate::jobs::testutil::a_fixer(&st);
        assert_eq!(
            work_tile(&st, fixer),
            st.world.as_ref().unwrap().spot_of(fixer)
        );
    }
}
