//! Legs: the walk in the place you are in, from you to the target or to the next way
//! through (a door, a lift, the stairs, a connector). Spec §2.

use super::Target;
use crate::app::AppState;
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
    /// The stairwell, to that level.
    Stairs {
        up: bool,
        to: i32,
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
    let player = st.player.as_ref().ok_or(NoLeg::NoWay)?;
    leg_from(st, player.place, player.pos, target)
}

/// The walk from tile `from` of `place` towards a concrete target, as `leg` makes it
/// from where you stand: the map's walk from your building's way out.
pub fn leg_from(st: &AppState, place: Place, from: Pos, target: Target) -> Result<Leg, NoLeg> {
    let city = st.city.as_ref().ok_or(NoLeg::NoWay)?;
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
    let aim = aim_of(target).ok_or(NoLeg::NoWay)?;
    lead_on(st, map, k, from, aim)
        .or_else(|| through_a_building(st, map, k, from, aim))
        .ok_or(NoLeg::NoWay)
}

/// The first two ways on from a stratum: the target's goal on `k` if a walk reaches
/// it, else a connector towards one of its other strata, the nearest first.
fn lead_on(st: &AppState, map: &MapRef, k: StratumKind, from: Pos, aim: Aim) -> Option<Leg> {
    let city = map.city();
    let strata = strata_of(city, aim);
    if strata.contains(&k) {
        let through = match aim {
            Aim::Building(b) if !walk_in(city, b) => Through::Door(b),
            _ => Through::Arrive,
        };
        // In a walk-in's footprint you are there.
        let inside = match aim {
            Aim::Building(b) => walk_in(city, b) && city.building(b).footprint.contains(from),
            Aim::Tile { .. } => false,
        };
        let found = if inside {
            Some((Vec::new(), from))
        } else {
            towards(map, from, &goals_on(map, aim, k), true, &[])
        };
        if let Some((path, goal)) = found {
            return Some(Leg {
                place: map.place(),
                path,
                goal,
                through,
            });
        }
    }
    let mut others: Vec<StratumKind> = strata.into_iter().filter(|s| *s != k).collect();
    others.sort_by_key(|s| (s.index().abs_diff(k.index()), s.index()));
    others
        .into_iter()
        .find_map(|t| between(st, map, k, t, from, aim))
}

/// The third way on from a stratum, off a rooftop with no ladder: the nearest closed
/// building's door on `k` you can walk to whose building has another door that leads
/// on. You go in and come out that way.
fn through_a_building(
    st: &AppState,
    map: &MapRef,
    k: StratumKind,
    from: Pos,
    aim: Aim,
) -> Option<Leg> {
    let city = map.city();
    let walked = field(map.size(), &[from], open(*map, true, &[], &[from]));
    let steps_to = |door: Pos| {
        if door == from {
            return Some(0);
        }
        door.neighbors4()
            .filter_map(|n| walked.get(n).copied())
            .filter(|d| *d != u32::MAX)
            .min()
            .map(|d| d + 1)
    };
    let mut doors: Vec<(u32, Pos, BuildingId)> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_closed())
        .flat_map(|b| {
            city.portals(b.id)
                .into_iter()
                .filter(|(s, _)| *s == k)
                .map(move |(_, p)| (p, b.id))
        })
        .filter_map(|(p, b)| Some((steps_to(p)?, p, b)))
        .collect();
    doors.sort_by_key(|(d, p, _)| (*d, *p));
    let mut fields: Vec<(StratumKind, Grid<u32>)> = Vec::new();
    for (_, door, b) in doors {
        let others = city.portals(b).into_iter().filter(|w| *w != (k, door));
        let on = others.into_iter().any(|(s, q)| {
            if !fields.iter().any(|(x, _)| *x == s) {
                fields.push((s, lead_field(st, city, s, aim)));
            }
            fields.iter().any(|(x, f)| *x == s && leads(f, q))
        });
        if on {
            let path = walk(map, from, door, true, &[])?;
            return Some(Leg {
                place: map.place(),
                path,
                goal: door,
                through: Through::Door(b),
            });
        }
    }
    None
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
        through: Through::Stairs { up: to > level, to },
    })
}

/// Out of the building by a door whose landing leads on to the target, the target's
/// stratum first: this floor's exit, or the floor that has one. When no door leads on,
/// the street door.
fn out_of(
    st: &AppState,
    map: &MapRef,
    (plan, floor): (&FloorPlan, &Floor),
    from: Pos,
    target: Target,
    exits: &[Pos],
) -> Result<Leg, NoLeg> {
    let city = map.city();
    let (b, level) = (plan.building, floor.level);
    let aim = aim_of(target).ok_or(NoLeg::NoWay)?;
    let t = nearest_stratum(&strata_of(city, aim), StratumKind::Street).ok_or(NoLeg::NoWay)?;
    // This floor's exits onto `k` through the city-map doors `good` allows.
    let exit = |k: StratumKind, good: &dyn Fn(Pos) -> bool| {
        let goals: Vec<Pos> = floor
            .exits
            .iter()
            .filter(|e| e.stratum == k && good(e.door))
            .map(|e| e.at)
            .collect();
        towards(map, from, &goals, false, exits).map(|(path, goal)| Leg {
            place: map.place(),
            path,
            goal,
            through: Through::Exit,
        })
    };
    let doors = city.portals(b);
    let mut ks: Vec<StratumKind> = doors.iter().map(|(k, _)| *k).collect();
    ks.sort_by_key(|k| (k.index().abs_diff(t.index()), k.index()));
    ks.dedup();
    for k in ks {
        let lead = lead_field(st, city, k, aim);
        let good: Vec<Pos> = doors
            .iter()
            .filter(|(s, p)| *s == k && leads(&lead, *p))
            .map(|(_, p)| *p)
            .collect();
        if good.is_empty() {
            continue;
        }
        if let Some(leg) = exit(k, &|p| good.contains(&p)) {
            return Ok(leg);
        }
        // The floor those doors open onto, unless it is known not to.
        let opens = |l: i32| {
            l != level
                && plan.has_level(l)
                && st.interiors.floor(b, l).is_none_or(|(_, f)| {
                    f.exits
                        .iter()
                        .any(|e| e.stratum == k && good.contains(&e.door))
                })
        };
        if let Some(l) = plan.door_level(k).filter(|l| opens(*l)) {
            return to_level(st, map, plan, level, l, from, exits);
        }
    }
    if let Some(leg) = exit(StratumKind::Street, &|_| true) {
        return Ok(leg);
    }
    if level != 0 {
        return to_level(st, map, plan, level, 0, from, exits);
    }
    Err(NoLeg::NoWay)
}

/// The next stratum from `s` towards `t`.
fn toward(s: StratumKind, t: StratumKind) -> Option<StratumKind> {
    if t.index() > s.index() {
        s.above()
    } else {
        s.below()
    }
}

/// The tiles of an aim on `k` that open a walk-in's doors to a walk.
fn ends(aim: Aim, k: StratumKind) -> Vec<Pos> {
    match aim {
        Aim::Tile { k: at, pos, .. } if at == k => vec![pos],
        _ => Vec::new(),
    }
}

/// Steps on stratum `j` to the target's goal there when `j` is `t`, else to a
/// connector on towards `t` whose landing is in the next stratum's field.
fn onward_field(st: &AppState, city: &City, j: StratumKind, t: StratumKind, aim: Aim) -> Grid<u32> {
    let map = MapRef::stratum_at(city, j, st.minute_of_day());
    let sources: Vec<Pos> = match toward(j, t) {
        _ if j == t => goals_on(&map, aim, j),
        Some(n) => {
            let on = onward_field(st, city, n, t, aim);
            links(city, j, n).filter(|p| reached(&on, *p)).collect()
        }
        None => Vec::new(),
    };
    field(map.size(), &sources, open(map, true, &[], &ends(aim, j)))
}

/// Steps on stratum `k` to its way on, as `lead_on` finds it from any tile: the
/// target's goal there, or a connector that leads on towards its other strata.
fn lead_field(st: &AppState, city: &City, k: StratumKind, aim: Aim) -> Grid<u32> {
    let map = MapRef::stratum_at(city, k, st.minute_of_day());
    let strata = strata_of(city, aim);
    let mut sources = if strata.contains(&k) {
        goals_on(&map, aim, k)
    } else {
        Vec::new()
    };
    for t in strata.into_iter().filter(|t| *t != k) {
        let Some(j) = toward(k, t) else { continue };
        let onward = onward_field(st, city, j, t, aim);
        sources.extend(links(city, k, j).filter(|p| reached(&onward, *p)));
    }
    field(map.size(), &sources, open(map, true, &[], &ends(aim, k)))
}

fn reached(f: &Grid<u32>, p: Pos) -> bool {
    f.get(p).is_some_and(|d| *d != u32::MAX)
}

/// Whether a city-map door leads on: it, or a tile beside it, is in the field.
fn leads(f: &Grid<u32>, door: Pos) -> bool {
    reached(f, door) || door.neighbors4().any(|n| reached(f, n))
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
) -> Option<Leg> {
    let city = map.city();
    let j = toward(k, t)?;
    let walked = field(map.size(), &[from], open(*map, true, &[], &[from]));
    let onward = onward_field(st, city, j, t, aim);
    let cost = |p: Pos| {
        (reached(&walked, p) && reached(&onward, p)).then(|| (walked[p] + onward[p], walked[p], p))
    };
    let (_, _, goal) = links(city, k, j).filter_map(cost).min()?;
    let path = walk(map, from, goal, true, &[])?;
    Some(Leg {
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
                Some(plan) => plan.door_level(k),
                None => FloorPlan::new(city, b).and_then(|plan| plan.door_level(k)),
            };
            Some(match up {
                Some(l) if Some(l) != door => format!("building, then {}", FloorPlan::label(l)),
                _ => "building".to_string(),
            })
        }
        Through::Lift(l) => Some(format!("lift to {}", FloorPlan::label(l))),
        Through::Stairs { up, .. } => {
            Some(if up { "stairs up" } else { "stairs down" }.to_string())
        }
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
pub(crate) fn open<'a>(
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
pub(crate) fn field(size: Size, sources: &[Pos], ok: impl Fn(Pos) -> bool) -> Grid<u32> {
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
            Some("building")
        );
        // A spot up in a building: the building, then its floor.
        let (b, level, _, tile) = crate::jobs::generate::first_fetch(&st).unwrap();
        assert!(level > 0);
        let fetch = spot(Place::Floor { building: b, level }, tile);
        let leg = super::leg(&st, fetch).unwrap();
        assert_eq!(leg.through, Through::Door(b));
        assert_eq!(
            next_text(&st, &leg, fetch),
            Some(format!("building, then {}", FloorPlan::label(level)))
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
        assert_eq!(
            leg.through,
            Through::Stairs {
                up: true,
                to: level
            }
        );
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
                Through::Lift(0) | Through::Stairs { up: false, to: 0 }
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

    #[test]
    fn a_walled_off_spot_has_no_way_and_no_door() {
        let mut st = awake();
        let from = outside_home(&mut st);
        put(&mut st, STREET, from);
        let to = steps_off(&street(&st), from, 30);
        let city = st.city.as_mut().unwrap();
        for p in to.neighbors8() {
            city.tile_mut(StratumKind::Street, p).unwrap().terrain = Terrain::Wall;
        }
        assert_eq!(leg(&st, spot(STREET, to)), Err(NoLeg::NoWay));
    }

    const HEIGHTS: Place = Place::Stratum(StratumKind::Heights);

    /// A closed building's roof door, its rooftop piece (steps from the door), and
    /// whether a ladder leaves that piece.
    struct Roof {
        b: BuildingId,
        door: Pos,
        piece: HashMap<Pos, u32>,
        ladder: bool,
    }

    impl Roof {
        /// A rooftop tile beside the door.
        fn beside(&self) -> Pos {
            let mut at: Vec<Pos> = self
                .piece
                .iter()
                .filter(|(_, d)| **d == 1)
                .map(|(p, _)| *p)
                .collect();
            at.sort();
            at[0]
        }
    }

    fn roofs(st: &AppState) -> Vec<Roof> {
        let city = st.city.as_ref().unwrap();
        let (h, sky) = (StratumKind::Heights, StratumKind::Skyways);
        let map = MapRef::stratum_of(city, h);
        city.buildings
            .iter()
            .filter(|b| b.kind.is_closed())
            .filter_map(|b| {
                let (_, door) = city.portals(b.id).into_iter().find(|(k, _)| *k == h)?;
                let piece = steps(&map, door);
                let ladder = city.connectors.iter().any(|c| {
                    c.public
                        && c.serves.contains(&h)
                        && c.serves.contains(&sky)
                        && piece.contains_key(&c.pos)
                });
                Some(Roof {
                    b: b.id,
                    door,
                    piece,
                    ladder,
                })
            })
            .collect()
    }

    #[test]
    fn a_roof_with_a_ladder_leads_off_it() {
        let mut st = awake();
        let roofs = roofs(&st);
        let here = roofs.iter().find(|r| r.ladder).unwrap();
        // A tower whose roof is not this one's.
        let tower = roofs
            .iter()
            .find(|r| r.b != here.b && !here.piece.contains_key(&r.door))
            .unwrap();
        put(&mut st, HEIGHTS, here.beside());
        let target = Target::Building(tower.b);
        let leg = leg(&st, target).unwrap();
        assert_eq!(leg.through, Through::Connector(StratumKind::Skyways));
        assert!(here.piece.contains_key(&leg.goal));
        assert_eq!(
            next_text(&st, &leg, target).as_deref(),
            Some("down to the Skyways")
        );
    }

    #[test]
    fn a_roof_with_no_ladder_goes_in_by_a_door() {
        let mut st = awake();
        let outside = outside_home(&mut st);
        let roofs = roofs(&st);
        let here = roofs.iter().find(|r| !r.ladder).unwrap();
        let on_it: Vec<BuildingId> = roofs
            .iter()
            .filter(|r| here.piece.contains_key(&r.door))
            .map(|r| r.b)
            .collect();
        put(&mut st, HEIGHTS, here.beside());
        let there = spot(STREET, outside);
        let leg = leg(&st, there).unwrap();
        let Through::Door(b) = leg.through else {
            panic!("{leg:?}")
        };
        assert!(on_it.contains(&b), "{b:?} is not on this roof");
        let city = st.city.as_ref().unwrap();
        assert!(city.portals(b).contains(&(StratumKind::Heights, leg.goal)));
        assert_eq!(next_text(&st, &leg, there).as_deref(), Some("building"));
        // Inside, the way leads down, not back up onto the roof.
        let top = st.interiors.plan(city, b).unwrap().top();
        st.interiors.ensure(city, b, top).unwrap();
        let (_, floor) = st.interiors.floor(b, top).unwrap();
        let roof = floor.exits.iter().find(|e| e.door == leg.goal).unwrap();
        let at = floor.doorstep(roof);
        put(
            &mut st,
            Place::Floor {
                building: b,
                level: top,
            },
            at,
        );
        let down = super::leg(&st, there).unwrap();
        assert!(
            matches!(
                down.through,
                Through::Lift(0) | Through::Stairs { up: false, to: 0 }
            ),
            "{down:?}"
        );
    }

    #[test]
    fn from_inside_a_tower_a_far_roof_is_not_up_its_own() {
        let mut st = awake();
        let roofs = roofs(&st);
        // A roof a ladder leaves, and a tower whose roof is not that one.
        let far = roofs.iter().find(|r| r.ladder).unwrap();
        let tower = roofs
            .iter()
            .find(|r| !far.piece.contains_key(&r.door))
            .unwrap()
            .b;
        let target = spot(HEIGHTS, far.beside());
        let city = st.city.as_ref().unwrap();
        let plan = st.interiors.plan(city, tower).unwrap().clone();
        st.interiors.ensure(city, tower, 0).unwrap();
        st.interiors.ensure(city, tower, plan.top()).unwrap();
        // On the ground floor: out, or to the bridge; not up to the roof.
        let (_, ground) = st.interiors.floor(tower, 0).unwrap();
        let at = ground.doorstep(&ground.exits[0]);
        let ways: Vec<(Pos, StratumKind)> =
            ground.exits.iter().map(|e| (e.at, e.stratum)).collect();
        let onto = |leg: &Leg| ways.iter().find(|(p, _)| *p == leg.goal).map(|(_, k)| *k);
        put(
            &mut st,
            Place::Floor {
                building: tower,
                level: 0,
            },
            at,
        );
        let leg = leg(&st, target).unwrap();
        match leg.through {
            Through::Exit => {
                assert_ne!(onto(&leg), Some(StratumKind::Heights), "{leg:?}")
            }
            Through::Lift(l) => {
                assert_eq!(Some(l), plan.door_level(StratumKind::Skyways), "{leg:?}")
            }
            _ => panic!("{leg:?}"),
        }
        // Beside the roof door upstairs: down, not out onto this roof.
        let (_, upstairs) = st.interiors.floor(tower, plan.top()).unwrap();
        let roof = upstairs
            .exits
            .iter()
            .find(|e| e.stratum == StratumKind::Heights)
            .unwrap();
        let at = upstairs.doorstep(roof);
        put(
            &mut st,
            Place::Floor {
                building: tower,
                level: plan.top(),
            },
            at,
        );
        let leg = super::leg(&st, target).unwrap();
        assert!(
            matches!(leg.through, Through::Lift(l) if l < plan.top())
                || matches!(leg.through, Through::Stairs { up: false, .. }),
            "{leg:?}"
        );
    }
}
