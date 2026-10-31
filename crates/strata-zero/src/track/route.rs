//! Legs: the walk in the place you are in, from you to the target or to the next way
//! through (a door, a lift, the stairs, a connector). Spec §2.

use super::Target;
use crate::app::AppState;
use crate::interiors::level_for;
use crate::render::map::{MapRef, TileMap};
use citygen::pathing::{astar, bfs_nearest};
use citygen::{
    BuildingId, City, ConnectorKind, Floor, FloorPlan, Grid, Place, Pos, Size, StratumKind, Terrain,
};
use std::collections::{HashSet, VecDeque};

/// The walk in one place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leg {
    pub place: Place,
    /// The tiles after yours, in walking order, ending on `goal`; empty when you stand
    /// on it.
    pub path: Vec<Pos>,
    pub goal: Pos,
    pub through: Through,
}

/// What waits at a leg's goal: the target, or the way on to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Through {
    Arrive,
    /// The building's door on this stratum.
    Door(BuildingId),
    /// The lift bank, to that level.
    Lift(i32),
    Stairs {
        up: bool,
    },
    /// This floor's door out.
    Exit,
    /// A connector to that stratum.
    Connector(StratumKind),
}

/// Why there is no leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoLeg {
    /// In a construct or on the Net grid.
    JackedIn,
    NoWay,
}

/// Where a target lies, seen from outside its building.
#[derive(Clone, Copy)]
enum Aim {
    /// A tile on a stratum, arrived at within `radius`.
    Tile {
        k: StratumKind,
        pos: Pos,
        radius: i32,
    },
    /// A building: its doors, or beside a walk-in's footprint.
    Building(BuildingId),
}

fn aim_of(target: Target) -> Option<Aim> {
    match target {
        Target::Spot {
            place: Place::Stratum(k),
            pos,
            radius,
        } if k != StratumKind::Net => Some(Aim::Tile { k, pos, radius }),
        Target::Spot {
            place: Place::Floor { building, .. },
            ..
        }
        | Target::Building(building) => Some(Aim::Building(building)),
        _ => None,
    }
}

/// A building you walk into on the city map: no door, no floors.
fn walk_in(city: &City, b: BuildingId) -> bool {
    city.building_opt(b).is_some_and(|b| !b.kind.is_closed())
}

/// The strata an aim is reached on.
fn strata_of(city: &City, aim: Aim) -> Vec<StratumKind> {
    let mut out: Vec<StratumKind> = match aim {
        Aim::Tile { k, .. } => vec![k],
        Aim::Building(b) if walk_in(city, b) => city
            .building_opt(b)
            .map(|x| {
                x.span
                    .iter()
                    .filter(|k| *k != StratumKind::Heights)
                    .collect()
            })
            .unwrap_or_default(),
        Aim::Building(b) => city.portals(b).into_iter().map(|(k, _)| k).collect(),
    };
    out.sort_by_key(|k| k.index());
    out.dedup();
    out
}

/// The one of `strata` nearest `k` in the stack, the lower on a tie.
fn nearest_stratum(strata: &[StratumKind], k: StratumKind) -> Option<StratumKind> {
    strata
        .iter()
        .copied()
        .min_by_key(|s| (s.index().abs_diff(k.index()), s.index()))
}

/// The walk from where you stand towards a concrete target (a `Spot` or a
/// `Building`; a `Person` is `locate`d first).
pub fn leg(st: &AppState, target: Target) -> Result<Leg, NoLeg> {
    let city = st.city.as_ref().ok_or(NoLeg::NoWay)?;
    let player = st.player.as_ref().ok_or(NoLeg::NoWay)?;
    let (place, from) = (player.place, player.pos);
    if matches!(
        place,
        Place::Construct(_) | Place::Stratum(StratumKind::Net)
    ) {
        return Err(NoLeg::JackedIn);
    }
    let map = MapRef::for_place(
        city,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    )
    .ok_or(NoLeg::NoWay)?;
    match place {
        Place::Stratum(k) => on_stratum(st, &map, k, from, target),
        Place::Floor { building, level } => on_floor(st, &map, building, level, from, target),
        Place::Construct(_) => Err(NoLeg::JackedIn),
    }
}

fn on_stratum(
    st: &AppState,
    map: &MapRef,
    k: StratumKind,
    from: Pos,
    target: Target,
) -> Result<Leg, NoLeg> {
    let city = map.city();
    let aim = aim_of(target).ok_or(NoLeg::NoWay)?;
    let strata = strata_of(city, aim);
    if !strata.contains(&k) {
        let t = nearest_stratum(&strata, k).ok_or(NoLeg::NoWay)?;
        return between(st, map, k, t, from, aim);
    }
    let through = match aim {
        Aim::Building(b) if !walk_in(city, b) => Through::Door(b),
        _ => Through::Arrive,
    };
    // In a walk-in's footprint you are there.
    let inside = match aim {
        Aim::Building(b) => walk_in(city, b) && city.building(b).footprint.contains(from),
        Aim::Tile { .. } => false,
    };
    let (path, goal) = if inside {
        (Vec::new(), from)
    } else {
        towards(map, from, &goals_on(map, aim, k), true, &[]).ok_or(NoLeg::NoWay)?
    };
    Ok(Leg {
        place: map.place(),
        path,
        goal,
        through,
    })
}

/// A spot's tile when a walk may stand on it, else the tiles within its radius (at
/// least beside it) that it may.
fn spot_goals(ok: impl Fn(Pos) -> bool, pos: Pos, radius: i32) -> Vec<Pos> {
    if ok(pos) {
        return vec![pos];
    }
    let r = radius.max(1);
    (-r..=r)
        .flat_map(|dy| (-r..=r).map(move |dx| pos.offset(dx, dy)))
        .filter(|p| *p != pos && ok(*p))
        .collect()
}

/// Where a walk on stratum `k` ends for an aim: the tile (or beside it), a building's
/// doors there, or the tiles beside a walk-in.
fn goals_on(map: &MapRef, aim: Aim, k: StratumKind) -> Vec<Pos> {
    let city = map.city();
    match aim {
        Aim::Tile { k: at, pos, radius } if at == k => {
            spot_goals(open(*map, true, &[], &[pos]), pos, radius)
        }
        Aim::Tile { .. } => Vec::new(),
        Aim::Building(b) if walk_in(city, b) => {
            let fp = city.building(b).footprint;
            let ok = open(*map, true, &[], &[]);
            fp.grow(1)
                .border()
                .filter(|p| p.neighbors4().any(|n| fp.contains(n)) && ok(*p))
                .collect()
        }
        Aim::Building(b) => city
            .portals(b)
            .into_iter()
            .filter(|(s, _)| *s == k)
            .map(|(_, p)| p)
            .collect(),
    }
}

fn on_floor(
    st: &AppState,
    map: &MapRef,
    b: BuildingId,
    level: i32,
    from: Pos,
    target: Target,
) -> Result<Leg, NoLeg> {
    let (plan, floor) = map.floor().ok_or(NoLeg::NoWay)?;
    let here = map.place();
    let exits: Vec<Pos> = floor.exits.iter().map(|e| e.at).collect();
    match target {
        Target::Spot { place, pos, radius } if place == here => {
            let goals = spot_goals(open(*map, false, &exits, &[]), pos, radius);
            let (path, goal) = towards(map, from, &goals, false, &exits).ok_or(NoLeg::NoWay)?;
            Ok(Leg {
                place: here,
                path,
                goal,
                through: Through::Arrive,
            })
        }
        Target::Building(x) if x == b => Ok(Leg {
            place: here,
            path: Vec::new(),
            goal: from,
            through: Through::Arrive,
        }),
        Target::Spot {
            place: Place::Floor {
                building,
                level: to,
            },
            ..
        } if building == b => to_level(st, map, plan, level, to, from, &exits),
        _ => out_of(st, map, (plan, floor), from, target, &exits),
    }
}

/// To the lift bank when the lifts answer, else to the stairwell.
fn to_level(
    st: &AppState,
    map: &MapRef,
    plan: &FloorPlan,
    level: i32,
    to: i32,
    from: Pos,
    exits: &[Pos],
) -> Result<Leg, NoLeg> {
    let place = map.place();
    let answer = st.net.lifts_answer(plan.building, st.net.now(st));
    if let Some(lift) = plan.lift.filter(|_| answer)
        && let Some(path) = walk(map, from, lift, false, exits)
    {
        return Ok(Leg {
            place,
            path,
            goal: lift,
            through: Through::Lift(to),
        });
    }
    let path = walk(map, from, plan.stair, false, exits).ok_or(NoLeg::NoWay)?;
    Ok(Leg {
        place,
        path,
        goal: plan.stair,
        through: Through::Stairs { up: to > level },
    })
}

/// Out of the building: this floor's exit onto the target's stratum, or the floor
/// that has one.
fn out_of(
    st: &AppState,
    map: &MapRef,
    (plan, floor): (&FloorPlan, &Floor),
    from: Pos,
    target: Target,
    exits: &[Pos],
) -> Result<Leg, NoLeg> {
    let city = map.city();
    let level = floor.level;
    let aim = aim_of(target).ok_or(NoLeg::NoWay)?;
    let t = nearest_stratum(&strata_of(city, aim), StratumKind::Street).ok_or(NoLeg::NoWay)?;
    let onto = |k: StratumKind| -> Vec<Pos> {
        floor
            .exits
            .iter()
            .filter(|e| e.stratum == k)
            .map(|e| e.at)
            .collect()
    };
    let exit = |goals: &[Pos]| {
        towards(map, from, goals, false, exits).map(|(path, goal)| Leg {
            place: map.place(),
            path,
            goal,
            through: Through::Exit,
        })
    };
    if let Some(leg) = exit(&onto(t)) {
        return Ok(leg);
    }
    // The floor that opens onto it, unless it is known not to.
    let opens = |l: i32| {
        l != level
            && plan.levels().any(|x| x == l)
            && st
                .interiors
                .floor(plan.building, l)
                .is_none_or(|(_, f)| f.exits.iter().any(|e| e.stratum == t))
    };
    if let Some(l) = level_for(plan, t).filter(|l| opens(*l)) {
        return to_level(st, map, plan, level, l, from, exits);
    }
    // Any way out, onto the stratum nearest it first.
    let mut ks: Vec<StratumKind> = floor.exits.iter().map(|e| e.stratum).collect();
    ks.sort_by_key(|k| (k.index().abs_diff(t.index()), k.index()));
    ks.dedup();
    if let Some(leg) = ks.into_iter().find_map(|k| exit(&onto(k))) {
        return Ok(leg);
    }
    if level != 0 {
        return to_level(st, map, plan, level, 0, from, exits);
    }
    Err(NoLeg::NoWay)
}

/// On stratum `k`, the target reached on `t`: to the connector on to the next stratum
/// that leads on, the least walk here plus walk there (spec §2, "Between strata").
fn between(
    st: &AppState,
    map: &MapRef,
    k: StratumKind,
    t: StratumKind,
    from: Pos,
    aim: Aim,
) -> Result<Leg, NoLeg> {
    let city = map.city();
    let next = |s: StratumKind| {
        if t.index() > s.index() {
            s.above()
        } else {
            s.below()
        }
    };
    let j = next(k).ok_or(NoLeg::NoWay)?;
    let here: Vec<Pos> = links(city, k, j).collect();
    if here.is_empty() {
        return Err(NoLeg::NoWay);
    }
    let walked = field(map.size(), &[from], open(*map, true, &[], &[from]));
    // On `j`, the walk from each landing to the target, or to a connector onward.
    let there = MapRef::stratum_at(city, j, st.minute_of_day());
    let (sources, ends) = match aim {
        Aim::Tile { pos, .. } if j == t => (goals_on(&there, aim, j), vec![pos]),
        _ if j == t => (goals_on(&there, aim, j), Vec::new()),
        _ => (
            next(j).map_or_else(Vec::new, |n| links(city, j, n).collect()),
            Vec::new(),
        ),
    };
    let onward = field(there.size(), &sources, open(there, true, &[], &ends));
    let cost = |p: Pos| {
        let (a, b) = (*walked.get(p)?, *onward.get(p)?);
        if a == u32::MAX || b == u32::MAX {
            return None;
        }
        Some((a + b, a, p))
    };
    let (_, _, goal) = here
        .into_iter()
        .filter_map(cost)
        .min()
        .ok_or(NoLeg::NoWay)?;
    let path = walk(map, from, goal, true, &[]).ok_or(NoLeg::NoWay)?;
    Ok(Leg {
        place: map.place(),
        path,
        goal,
        through: Through::Connector(j),
    })
}

/// The public connectors between strata `a` and `b`, standable on both.
fn links(city: &City, a: StratumKind, b: StratumKind) -> impl Iterator<Item = Pos> + '_ {
    city.connectors
        .iter()
        .filter(move |c| {
            c.public
                && c.kind != ConnectorKind::Jack
                && c.serves.contains(&a)
                && c.serves.contains(&b)
                && city.walkable(a, c.pos)
                && city.walkable(b, c.pos)
        })
        .map(|c| c.pos)
}

/// What the pane says comes after this leg; nothing when it arrives.
pub fn next_text(st: &AppState, leg: &Leg, target: Target) -> Option<String> {
    let city = st.city.as_ref()?;
    match leg.through {
        Through::Arrive => None,
        Through::Door(b) => {
            let up = match target {
                Target::Spot {
                    place: Place::Floor { building, level },
                    ..
                } if building == b => Some(level),
                _ => None,
            };
            let k = leg.place.stratum()?;
            let door = match st.interiors.plan_of(b) {
                Some(plan) => level_for(plan, k),
                None => FloorPlan::new(city, b).and_then(|plan| level_for(&plan, k)),
            };
            Some(match up {
                Some(l) if Some(l) != door => format!("door, then {}", FloorPlan::label(l)),
                _ => "door".to_string(),
            })
        }
        Through::Lift(l) => Some(format!("lift to {}", FloorPlan::label(l))),
        Through::Stairs { up } => Some(if up { "stairs up" } else { "stairs down" }.to_string()),
        Through::Exit => {
            let Place::Floor { building, level } = leg.place else {
                return None;
            };
            let (_, floor) = st.interiors.floor(building, level)?;
            let e = floor.exits.iter().find(|e| e.at == leg.goal)?;
            Some(format!("out to the {}", e.stratum.label().to_lowercase()))
        }
        Through::Connector(j) => {
            let k = leg.place.stratum()?;
            let way = if j.index() > k.index() { "up" } else { "down" };
            Some(format!("{way} to the {}", j.label()))
        }
    }
}

/// Whether a walk may step on a tile (spec §2): a walkable one, and of the doors on a
/// stratum (`doors_are_walls`) only those of a walk-in that one of `ends` lies in; on
/// a floor every door but its `exits`.
fn open<'a>(
    map: MapRef<'a>,
    doors_are_walls: bool,
    exits: &'a [Pos],
    ends: &[Pos],
) -> impl Fn(Pos) -> bool + 'a {
    let city = map.city();
    let inside: Vec<BuildingId> = ends
        .iter()
        .filter_map(|p| map.tile(*p)?.building)
        .filter(|b| walk_in(city, *b))
        .collect();
    move |p| {
        let Some(t) = map.tile(p).filter(|t| t.is_walkable()) else {
            return false;
        };
        if t.terrain != Terrain::Door {
            true
        } else if doors_are_walls {
            t.building.is_some_and(|b| inside.contains(&b))
        } else {
            !exits.contains(&p)
        }
    }
}

/// The shared A*: the tiles after `from` ending on `to`. On a stratum
/// (`doors_are_walls`) every door but `to` is a wall; on a floor only `exits` are.
pub(crate) fn walk(
    map: &MapRef,
    from: Pos,
    to: Pos,
    doors_are_walls: bool,
    exits: &[Pos],
) -> Option<Vec<Pos>> {
    if from == to {
        return Some(Vec::new());
    }
    let ok = open(*map, doors_are_walls, exits, &[from, to]);
    let end = map.walkable(to);
    let path = astar(map.size(), from, to, |_, n| {
        ((n == to && end) || ok(n)).then_some(1)
    })?;
    Some(path.into_iter().skip(1).collect())
}

/// The walk to the nearest of `goals` by the same rule, and which one it reaches.
fn towards(
    map: &MapRef,
    from: Pos,
    goals: &[Pos],
    doors_are_walls: bool,
    exits: &[Pos],
) -> Option<(Vec<Pos>, Pos)> {
    match goals {
        [] => None,
        [to] => walk(map, from, *to, doors_are_walls, exits).map(|path| (path, *to)),
        _ if goals.contains(&from) => Some((Vec::new(), from)),
        _ => {
            let set: HashSet<Pos> = goals.iter().copied().filter(|p| map.walkable(*p)).collect();
            let ends: Vec<Pos> = std::iter::once(from).chain(goals.iter().copied()).collect();
            let ok = open(*map, doors_are_walls, exits, &ends);
            let path = bfs_nearest(map.size(), from, &ok, |p| set.contains(&p), usize::MAX)?;
            let goal = *path.last()?;
            Some((path.into_iter().skip(1).collect(), goal))
        }
    }
}

/// Steps from the nearest of `sources` to every tile `ok` lets a walk on.
fn field(size: Size, sources: &[Pos], ok: impl Fn(Pos) -> bool) -> Grid<u32> {
    let mut d = Grid::filled(size, u32::MAX);
    let mut queue = VecDeque::new();
    for s in sources {
        if d.get(*s) == Some(&u32::MAX) {
            d.set(*s, 0);
            queue.push_back(*s);
        }
    }
    while let Some(p) = queue.pop_front() {
        let next = d[p] + 1;
        for n in p.neighbors4() {
            if d.get(n) == Some(&u32::MAX) && ok(n) {
                d.set(n, next);
                queue.push_back(n);
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::render::map::TileMap;
    use crate::screens::explore::MapScreen;
    use crate::track::flat_tile;
    use citygen::{BuildingKind, ConnectorKind, FloorPlan, Terrain};
    use std::collections::{HashMap, VecDeque};

    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// The home city, awake in your flat.
    fn awake() -> AppState {
        let mut st = state_home();
        MapScreen::walking(&mut st);
        st
    }

    fn put(st: &mut AppState, place: Place, pos: Pos) {
        let p = st.player.as_mut().unwrap();
        p.place = place;
        p.pos = pos;
    }

    fn spot(place: Place, pos: Pos) -> Target {
        Target::Spot {
            place,
            pos,
            radius: 1,
        }
    }

    /// The home megablock, its ground floor generated.
    fn home_lobby(st: &mut AppState) -> (BuildingId, Place) {
        let b = st.home.unwrap().home.building;
        let city = st.city.as_ref().unwrap();
        st.interiors.ensure(city, b, 0).unwrap();
        (
            b,
            Place::Floor {
                building: b,
                level: 0,
            },
        )
    }

    /// The street tile you land on leaving the home megablock.
    fn outside_home(st: &mut AppState) -> Pos {
        let (b, _) = home_lobby(st);
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        floor
            .exits
            .iter()
            .find(|e| e.stratum == StratumKind::Street)
            .unwrap()
            .landing
    }

    fn street(st: &AppState) -> MapRef<'_> {
        MapRef::stratum_of(st.city.as_ref().unwrap(), StratumKind::Street)
    }

    fn terrain(map: &MapRef, p: Pos) -> Terrain {
        map.tile(p).unwrap().terrain
    }

    /// Steps from `from` over walkable tiles that are not doors, outside buildings.
    fn steps(map: &MapRef, from: Pos) -> HashMap<Pos, u32> {
        let mut d = HashMap::from([(from, 0)]);
        let mut q = VecDeque::from([from]);
        while let Some(p) = q.pop_front() {
            for n in p.neighbors4() {
                let open = map
                    .tile(n)
                    .is_some_and(|t| t.is_walkable() && t.terrain != Terrain::Door);
                if open && !d.contains_key(&n) {
                    d.insert(n, d[&p] + 1);
                    q.push_back(n);
                }
            }
        }
        d
    }

    /// A tile outside every building exactly `n` steps from `from`.
    fn steps_off(map: &MapRef, from: Pos, n: u32) -> Pos {
        let d = steps(map, from);
        let mut at: Vec<Pos> = d
            .iter()
            .filter(|(p, k)| **k == n && map.tile(**p).unwrap().building.is_none())
            .map(|(p, _)| *p)
            .collect();
        at.sort();
        at[0]
    }

    /// Every step is to a 4-neighbour you can stand on.
    fn assert_walk(map: &MapRef, from: Pos, path: &[Pos]) {
        let mut at = from;
        for p in path {
            assert_eq!(at.manhattan(*p), 1, "{at:?} to {p:?}");
            assert!(map.walkable(*p), "{p:?}");
            at = *p;
        }
    }

    #[test]
    fn same_place_walks_to_the_tile() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let map = street(&st);
        let to = steps_off(&map, from, 20);
        let leg = leg(&st, spot(STREET, to)).unwrap();
        assert_eq!(leg.place, STREET);
        assert_eq!(leg.through, Through::Arrive);
        assert_eq!(leg.goal, to);
        assert_eq!(leg.path.last(), Some(&to));
        assert_eq!(leg.path.len(), 20);
        assert_walk(&map, from, &leg.path);
        assert_eq!(next_text(&st, &leg, spot(STREET, to)), None);
        // Standing on it, the walk is empty.
        put(&mut st, STREET, to);
        let here = super::leg(&st, spot(STREET, to)).unwrap();
        assert!(here.path.is_empty());
        assert_eq!(here.goal, to);
    }

    #[test]
    fn an_unwalkable_spot_walks_beside_it() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let map = street(&st);
        let city = st.city.as_ref().unwrap();
        let fp = city.building(st.home.unwrap().home.building).footprint;
        let open = |p: Pos| {
            map.tile(p)
                .is_some_and(|t| t.is_walkable() && t.terrain != Terrain::Door)
        };
        // The building's wall nearest you with open ground beside it.
        let wall = fp
            .border()
            .filter(|p| terrain(&map, *p) == Terrain::Wall)
            .filter(|p| p.neighbors4().any(open))
            .min_by_key(|p| (p.manhattan(from), *p))
            .unwrap();
        let leg = leg(&st, spot(STREET, wall)).unwrap();
        assert_eq!(leg.through, Through::Arrive);
        assert_eq!(leg.goal.chebyshev(wall), 1);
        assert!(open(leg.goal));
        assert_eq!(leg.path.last(), Some(&leg.goal));
        assert_walk(&map, from, &leg.path);
        // No other tile beside it is a shorter walk.
        let d = steps(&map, from);
        assert_eq!(d[&leg.goal] as usize, leg.path.len());
        for q in wall.neighbors8().filter(|q| open(*q)) {
            if let Some(n) = d.get(&q) {
                assert!(leg.path.len() <= *n as usize, "{q:?} is nearer");
            }
        }
    }

    #[test]
    fn street_to_a_building_goes_to_its_door() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let map = street(&st);
        let city = st.city.as_ref().unwrap();
        let clinic = city
            .buildings
            .iter()
            .map(|b| b.id)
            .find(|b| crate::clinic::is_clinic(&st, *b))
            .unwrap();
        let leg = leg(&st, Target::Building(clinic)).unwrap();
        assert_eq!(leg.through, Through::Door(clinic));
        assert!(
            city.portals(clinic)
                .contains(&(StratumKind::Street, leg.goal))
        );
        assert_eq!(leg.path.last(), Some(&leg.goal));
        assert_walk(&map, from, &leg.path);
        assert_eq!(
            next_text(&st, &leg, Target::Building(clinic)).as_deref(),
            Some("door")
        );
        // A spot up in a building: its door, then its floor.
        let (b, level, _, tile) = crate::jobs::generate::first_fetch(&st).unwrap();
        assert!(level > 0);
        let fetch = spot(Place::Floor { building: b, level }, tile);
        let leg = super::leg(&st, fetch).unwrap();
        assert_eq!(leg.through, Through::Door(b));
        assert_eq!(
            next_text(&st, &leg, fetch),
            Some(format!("door, then {}", FloorPlan::label(level)))
        );
    }

    #[test]
    fn a_walk_in_kind_goes_beside_its_footprint() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let map = street(&st);
        let city = st.city.as_ref().unwrap();
        let bar = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::NoodleBar)
            .min_by_key(|b| b.footprint.center().manhattan(from))
            .unwrap();
        assert!(city.portals(bar.id).is_empty(), "a walk-in has no door");
        let fp = bar.footprint;
        let leg = leg(&st, Target::Building(bar.id)).unwrap();
        assert_eq!(leg.through, Through::Arrive);
        assert!(map.walkable(leg.goal));
        assert!(!fp.contains(leg.goal));
        assert!(leg.goal.neighbors4().any(|n| fp.contains(n)));
        assert_walk(&map, from, &leg.path);
    }

    #[test]
    fn a_walk_in_lets_you_through_its_own_door() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let map = street(&st);
        let city = st.city.as_ref().unwrap();
        let bar = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::NoodleBar)
            .min_by_key(|b| b.footprint.center().manhattan(from))
            .unwrap();
        let fp = bar.footprint;
        let inside = fp
            .inset(1)
            .iter()
            .find(|p| map.walkable(*p) && terrain(&map, *p) != Terrain::Door)
            .unwrap();
        // Only the bar's own doors.
        let ours = |map: &MapRef, path: &[Pos]| {
            path.iter()
                .all(|p| terrain(map, *p) != Terrain::Door || fp.contains(*p))
        };
        // In, through its door.
        let leg = leg(&st, spot(STREET, inside)).unwrap();
        assert_eq!(leg.goal, inside);
        assert!(leg.path.iter().any(|p| terrain(&map, *p) == Terrain::Door));
        assert!(ours(&map, &leg.path), "{:?}", leg.path);
        // And out again.
        put(&mut st, STREET, inside);
        let map = street(&st);
        let out = super::leg(&st, spot(STREET, from)).unwrap();
        assert_eq!(out.goal, from);
        assert_walk(&map, inside, &out.path);
        assert!(ours(&map, &out.path), "{:?}", out.path);
    }

    #[test]
    fn a_walk_never_crosses_another_door() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let map = street(&st);
        let to = steps_off(&map, from, 150);
        let leg = leg(&st, spot(STREET, to)).unwrap();
        assert_eq!(leg.path.len(), 150);
        for p in &leg.path[..leg.path.len() - 1] {
            assert_ne!(terrain(&map, *p), Terrain::Door, "{p:?}");
        }
        // The shared walk keeps to the rule too.
        let path = walk(&map, from, to, true, &[]).unwrap();
        assert!(path.iter().all(|p| terrain(&map, *p) != Terrain::Door));
    }

    #[test]
    fn floor_to_another_floor_takes_the_lift() {
        let mut st = awake();
        let (b, lobby) = home_lobby(&mut st);
        let (plan, floor) = st.interiors.floor(b, 0).unwrap();
        let (lift, stair) = (plan.lift.unwrap(), plan.stair);
        let door = floor
            .exits
            .iter()
            .find(|e| e.stratum == StratumKind::Street)
            .unwrap();
        let start = floor.doorstep(door);
        put(&mut st, lobby, start);
        let (flat, pos) = flat_tile(&st).unwrap();
        let Place::Floor { level, .. } = flat else {
            unreachable!()
        };
        let home = Target::Spot {
            place: flat,
            pos,
            radius: 0,
        };
        let leg = leg(&st, home).unwrap();
        assert_eq!(leg.place, lobby);
        assert_eq!(leg.through, Through::Lift(level));
        assert_eq!(leg.goal, lift);
        assert_eq!(leg.path.last(), Some(&lift));
        assert_eq!(
            next_text(&st, &leg, home),
            Some(format!("lift to {}", FloorPlan::label(level)))
        );
        // Locked down, the stairs.
        let now = st.net.now(&st);
        st.net.trace(b).add(game::ALARM_HEAT, now);
        st.net.alarm(b, now);
        assert!(!st.net.lifts_answer(b, now));
        let leg = super::leg(&st, home).unwrap();
        assert_eq!(leg.through, Through::Stairs { up: true });
        assert_eq!(leg.goal, stair);
        assert_eq!(next_text(&st, &leg, home).as_deref(), Some("stairs up"));
    }

    #[test]
    fn home_from_the_lobby_stays_inside() {
        let mut st = awake();
        let (b, lobby) = home_lobby(&mut st);
        let (flat, pos) = flat_tile(&st).unwrap();
        let home = Target::Spot {
            place: flat,
            pos,
            radius: 0,
        };
        let (plan, floor) = st.interiors.floor(b, 0).unwrap();
        let lift = plan.lift.unwrap();
        let steps: Vec<Pos> = floor.exits.iter().map(|e| floor.doorstep(e)).collect();
        assert!(!steps.is_empty());
        // From beside every door of the lobby, the lift; never out.
        for at in steps {
            put(&mut st, lobby, at);
            let leg = leg(&st, home).unwrap();
            assert!(matches!(leg.through, Through::Lift(_)), "{at:?}: {leg:?}");
            assert_eq!(leg.goal, lift);
        }
        // Off the lift on the flat's floor, the flat.
        put(&mut st, flat, lift);
        let leg = leg(&st, home).unwrap();
        assert_eq!(leg.through, Through::Arrive);
        assert_eq!(leg.goal, pos);
    }

    #[test]
    fn floor_to_the_street_takes_the_exit() {
        let mut st = awake();
        let outside = outside_home(&mut st);
        let (b, lobby) = home_lobby(&mut st);
        let (plan, floor) = st.interiors.floor(b, 0).unwrap();
        let lift = plan.lift.unwrap();
        let doors: Vec<Pos> = floor
            .exits
            .iter()
            .filter(|e| e.stratum == StratumKind::Street)
            .map(|e| e.at)
            .collect();
        put(&mut st, lobby, lift);
        let there = spot(STREET, outside);
        let leg = leg(&st, there).unwrap();
        assert_eq!(leg.through, Through::Exit);
        assert!(doors.contains(&leg.goal), "{leg:?}");
        assert_eq!(leg.path.last(), Some(&leg.goal));
        assert_eq!(
            next_text(&st, &leg, there).as_deref(),
            Some("out to the street")
        );
        // From upstairs, down first.
        let (flat, _) = flat_tile(&st).unwrap();
        put(&mut st, flat, lift);
        let leg = super::leg(&st, there).unwrap();
        assert!(
            matches!(
                leg.through,
                Through::Lift(0) | Through::Stairs { up: false }
            ),
            "{leg:?}"
        );
        assert_eq!(next_text(&st, &leg, there).as_deref(), Some("lift to L"));
        // A building outside: out first.
        let city = st.city.as_ref().unwrap();
        let clinic = city
            .buildings
            .iter()
            .map(|b| b.id)
            .find(|b| crate::clinic::is_clinic(&st, *b))
            .unwrap();
        put(&mut st, lobby, lift);
        let leg = super::leg(&st, Target::Building(clinic)).unwrap();
        assert_eq!(leg.through, Through::Exit);
        assert!(doors.contains(&leg.goal), "{leg:?}");
    }

    #[test]
    fn a_fetch_behind_a_flat_door_is_reached() {
        let mut st = awake();
        let (b, level, _, tile) = crate::jobs::generate::first_fetch(&st).unwrap();
        let city = st.city.as_ref().unwrap();
        st.interiors.ensure(city, b, level).unwrap();
        let (plan, floor) = st.interiors.floor(b, level).unwrap();
        let start = plan.lift.unwrap_or(plan.stair);
        let exits: Vec<Pos> = floor.exits.iter().map(|e| e.at).collect();
        let place = Place::Floor { building: b, level };
        put(&mut st, place, start);
        let fetch = spot(place, tile);
        let leg = leg(&st, fetch).unwrap();
        assert_eq!(leg.place, place);
        assert_eq!(leg.through, Through::Arrive);
        assert!(leg.goal.chebyshev(tile) <= 1);
        let city = st.city.as_ref().unwrap();
        let map = MapRef::for_place(city, &st.interiors, &st.constructs, place, 0).unwrap();
        assert_walk(&map, start, &leg.path);
        assert!(
            leg.path.iter().any(|p| terrain(&map, *p) == Terrain::Door),
            "through the flat's door"
        );
        assert!(leg.path.iter().all(|p| !exits.contains(p)));
    }

    #[test]
    fn between_strata_takes_a_connector() {
        let mut st = awake();
        let outside = outside_home(&mut st);
        let (under, street) = (StratumKind::Undercity, StratumKind::Street);
        let city = st.city.as_ref().unwrap();
        let both = |c: &citygen::Connector| c.serves.contains(&under) && c.serves.contains(&street);
        let link = city
            .connectors
            .iter()
            .find(|c| {
                c.public
                    && c.kind != ConnectorKind::Jack
                    && both(c)
                    && city.walkable(under, c.pos)
                    && city.walkable(street, c.pos)
            })
            .unwrap()
            .pos;
        put(&mut st, Place::Stratum(under), link);
        let up = spot(STREET, outside);
        let leg = leg(&st, up).unwrap();
        assert_eq!(leg.place, Place::Stratum(under));
        assert_eq!(leg.through, Through::Connector(street));
        let city = st.city.as_ref().unwrap();
        assert!(city.connectors_at(leg.goal).any(both), "{leg:?}");
        assert_eq!(
            next_text(&st, &leg, up).as_deref(),
            Some("up to the Street")
        );
        // And back down.
        put(&mut st, STREET, outside);
        let down = spot(Place::Stratum(under), link);
        let leg = super::leg(&st, down).unwrap();
        assert_eq!(leg.through, Through::Connector(under));
        let city = st.city.as_ref().unwrap();
        assert!(city.connectors_at(leg.goal).any(both), "{leg:?}");
        assert_eq!(
            next_text(&st, &leg, down).as_deref(),
            Some("down to the Undercity")
        );
        // Two strata up, the Street first, at a link that leads on.
        let sky = StratumKind::Skyways;
        let city = st.city.as_ref().unwrap();
        let high = city
            .connectors
            .iter()
            .find(|c| {
                c.public
                    && c.serves.contains(&street)
                    && c.serves.contains(&sky)
                    && city.walkable(sky, c.pos)
            })
            .unwrap()
            .pos;
        put(&mut st, Place::Stratum(under), link);
        let leg = super::leg(&st, spot(Place::Stratum(sky), high)).unwrap();
        assert_eq!(leg.through, Through::Connector(street));
        let city = st.city.as_ref().unwrap();
        assert!(city.connectors_at(leg.goal).any(both), "{leg:?}");
    }

    #[test]
    fn a_construct_has_no_leg() {
        let mut st = awake();
        let outside = outside_home(&mut st);
        let b = st.home.unwrap().home.building;
        put(&mut st, Place::Construct(b), Pos::new(1, 1));
        assert_eq!(leg(&st, spot(STREET, outside)), Err(NoLeg::JackedIn));
        put(&mut st, Place::Stratum(StratumKind::Net), outside);
        assert_eq!(leg(&st, spot(STREET, outside)), Err(NoLeg::JackedIn));
        // A person is located first.
        put(&mut st, STREET, outside);
        let p = crate::jobs::testutil::a_fixer(&st);
        assert_eq!(leg(&st, Target::Person(p)), Err(NoLeg::NoWay));
    }
}
