//! Commuting: someone who works in the city with no home in it lives off the map. They
//! come in by a road where it crosses the map's edge, walk to work, and walk back out the
//! same way; so do they when a trip of theirs ends at no place in the city (a bar the
//! city hasn't got). Someone who lives in town has their drink in town. Off the map they
//! are `Whereabouts::Away`, drawn nowhere.

use crate::paths::walk_cost;
use crate::people::PersonId;
use citygen::rng::hash64;
use citygen::world::{City, StratumKind};
use citygen::{Grid, Pos, Terrain};
use std::collections::{HashSet, VecDeque};

/// Tiles a walker covers in a game minute (`WALK_TICKS` a tile, `TICKS_PER_MINUTE` a
/// minute).
const TILES_A_MINUTE: i32 = (crate::world::TICKS_PER_MINUTE / crate::world::WALK_TICKS) as i32;

/// What spreads people over the ways in.
const WAY: u64 = 0xC0_33E7;

fn carries_traffic(city: &City, p: Pos) -> bool {
    city.tile(StratumKind::Street, p)
        .is_some_and(|t| matches!(t.terrain, Terrain::Road | Terrain::Bridge))
}

/// How far in from the map's edge a road runs where it crosses the edge, at least: a
/// road along the edge, two lanes wide, isn't a way in.
const CROSSING: i32 = 4;

/// The step in from `p` on the map's edge, none off the edge.
fn inward(city: &City, p: Pos) -> Option<(i32, i32)> {
    let (w, h) = (city.size.w as i32, city.size.h as i32);
    match p {
        _ if p.x == 0 => Some((1, 0)),
        _ if p.x == w - 1 => Some((-1, 0)),
        _ if p.y == 0 => Some((0, 1)),
        _ if p.y == h - 1 => Some((0, -1)),
        _ => None,
    }
}

/// Whether a road crosses the map's edge at `p`: road (or a bridge) there and on the
/// cells running in from it.
pub fn crosses_the_edge(city: &City, p: Pos) -> bool {
    inward(city, p).is_some_and(|(dx, dy)| {
        (0..CROSSING).all(|k| carries_traffic(city, p.offset(dx * k, dy * k)))
    })
}

/// The map's edge, once round from the top-left corner.
fn edge(city: &City) -> impl Iterator<Item = Pos> {
    let (w, h) = (city.size.w as i32, city.size.h as i32);
    let top = (0..w).map(|x| Pos::new(x, 0));
    let right = (1..h).map(move |y| Pos::new(w - 1, y));
    let bottom = (0..w - 1).rev().map(move |x| Pos::new(x, h - 1));
    let left = (1..h - 1).rev().map(|y| Pos::new(0, y));
    top.chain(right).chain(bottom).chain(left)
}

/// Where a walk from the ways in goes: each cell's walk, numbered in order round the
/// edge (ways a walk joins share one), or none. One flood for each walk, made once with
/// the city's places.
#[derive(Default)]
pub struct Walks {
    walk: Option<Grid<u16>>,
}

impl Walks {
    fn of(&self, p: Pos) -> Option<u16> {
        self.walk.as_ref()?.get(p).copied().filter(|w| *w != 0)
    }

    /// Whether a walk from way in `way` reaches `to`.
    pub fn reaches(&self, way: Pos, to: Pos) -> bool {
        self.of(way).is_some_and(|w| self.of(to) == Some(w))
    }
}

/// Where people come in from off the map: the cells of the map's edge a road crosses
/// (`crosses_the_edge`) from which a walk reaches a street door, in order round the edge
/// (none for a map no road leaves); and where a walk from each goes.
pub fn ways_in(city: &City, doors: &HashSet<Pos>) -> (Vec<Pos>, Walks) {
    let walkable = |q: Pos| walk_cost(city, q, q).is_some();
    let mut walk = Grid::filled(city.size, 0u16);
    // whether walk n (from 1) reaches a door
    let mut to_a_door = vec![false];
    let crossings: Vec<Pos> = edge(city).filter(|p| crosses_the_edge(city, *p)).collect();
    for &p in &crossings {
        if walk[p] != 0 {
            continue;
        }
        let n = to_a_door.len() as u16;
        let mut door = false;
        let mut queue = VecDeque::from([p]);
        walk.set(p, n);
        while let Some(q) = queue.pop_front() {
            door |= doors.contains(&q);
            for r in q.neighbors4() {
                if walk.get(r) == Some(&0) && walkable(r) {
                    walk.set(r, n);
                    queue.push_back(r);
                }
            }
        }
        to_a_door.push(door);
    }
    let ways = crossings
        .into_iter()
        .filter(|p| to_a_door[walk[*p] as usize])
        .collect();
    (ways, Walks { walk: Some(walk) })
}

/// The way in `who` comes and goes by, of `ways`, the same every day: none when there's
/// none.
pub fn way_in_of(ways: &[Pos], who: PersonId) -> Option<Pos> {
    if ways.is_empty() {
        return None;
    }
    let i = hash64(WAY, u64::from(who.0)) % ways.len() as u64;
    Some(ways[i as usize])
}

/// One in this many who come in to work a building take a break at midday.
pub const LUNCH_ONE_IN: u64 = 3;
/// The earliest a break begins, and how far after it the last one does (12:00–13:30).
pub const LUNCH_FROM: u16 = 12 * 60;
pub const LUNCH_SPREAD: u16 = 90;
/// How long a break lasts, from the shortest to the longest.
pub const LUNCH_FOR: (u16, u16) = (30, 60);
/// How far from work (Manhattan, cells) somewhere to eat may be; past it, a spot along
/// the street near work does.
pub const LUNCH_REACH: i32 = 60;
/// A spot along the street for a break: this near work, and no nearer.
pub const LUNCH_SPOT: (i32, i32) = (3, 8);

/// One in this many who come in to work stay on for a drink after.
pub const DRINK_ONE_IN: u64 = 3;
/// The earliest they walk out after a drink, and how far after it the last does
/// (21:00–01:00).
pub const DRINK_UNTIL: u16 = 21 * 60;
pub const DRINK_SPREAD: u16 = 240;
/// Who knocks off between these (a day's work) may stay for a drink.
pub const DRINK_AFTER: (u16, u16) = (14 * 60, 21 * 60);

/// What spreads breaks and drinks over people and the hour.
const LUNCH: u64 = 0x1C_4C4;
const DRINK: u64 = 0xD2_14C;

/// Whether `who` takes a break at midday, and when and for how long (minute of the day
/// it begins, minutes): one in `LUNCH_ONE_IN`, by a hash of who they are.
pub fn lunch_of(who: PersonId) -> Option<(u16, u16)> {
    let h = |k: u64| hash64(LUNCH ^ k, u64::from(who.0));
    if h(0) % LUNCH_ONE_IN != 0 {
        return None;
    }
    let from = LUNCH_FROM + (h(1) % (u64::from(LUNCH_SPREAD) + 1)) as u16;
    let span = u64::from(LUNCH_FOR.1 - LUNCH_FOR.0) + 1;
    Some((from, LUNCH_FOR.0 + (h(2) % span) as u16))
}

/// Whether `who` stays for a drink after work, and the minute of the day they walk out:
/// one in `DRINK_ONE_IN`, by a hash of who they are.
pub fn drink_until(who: PersonId) -> Option<u16> {
    let h = |k: u64| hash64(DRINK ^ k, u64::from(who.0));
    if h(0) % DRINK_ONE_IN != 0 {
        return None;
    }
    let late = (h(1) % (u64::from(DRINK_SPREAD) + 1)) as u16;
    Some((DRINK_UNTIL + late) % crate::content::DAY)
}

/// One of `n` for `who`, by a hash of who they are and `salt`: the city's dice are left
/// alone.
pub fn pick_for(who: PersonId, salt: u64, n: usize) -> usize {
    (hash64(salt, u64::from(who.0)) % n.max(1) as u64) as usize
}

/// Whether `minute` of the day lies in `from` up to (not at) `to`, over midnight too.
pub fn between(minute: u16, from: u16, to: u16) -> bool {
    let day = crate::content::DAY;
    (minute + day - from) % day < (to + day - from) % day
}

/// Game minutes a walk from `from` to `to` takes, about.
pub fn walk_minutes(from: Pos, to: Pos) -> u16 {
    let per = TILES_A_MINUTE.max(1);
    ((from.manhattan(to).max(0) + per - 1) / per) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::People;
    use crate::people::{Dest, Goal, SimParams, Whereabouts, Work};
    use crate::places::Places;
    use crate::world::{TICKS_PER_MINUTE, World};
    use citygen::world::{BuildingKind, Theme};
    use citygen::{GenParams, Generator, SizePreset};
    use std::collections::HashMap;

    /// Seed 6 at five districts: most who work in it have no home in it.
    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    /// `c`'s people at `minute` of the day.
    fn world(c: &City, minute: u16) -> World {
        World::seed(
            c,
            &People::load().unwrap(),
            &SimParams::default(),
            minute,
            0,
        )
    }

    /// `w` run on a minute, every walk asked for found.
    fn a_minute(w: &mut World, c: &City) {
        let t = w.tick() + TICKS_PER_MINUTE;
        w.advance(c, t);
        w.path_some(c, usize::MAX);
    }

    fn on_the_edge(c: &City, p: Pos) -> bool {
        let (w, h) = (c.size.w as i32, c.size.h as i32);
        p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1
    }

    /// Those with no home in `w`'s city who work its buildings from `from` to `to` (minutes
    /// of the day their shift starts).
    fn day_workers(w: &World, from: u16, to: u16) -> Vec<usize> {
        w.people()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.home.is_none() && matches!(p.work, Work::Building(_)))
            .filter(|(_, p)| (from..=to).contains(&p.shift_start))
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn the_ways_in_are_where_roads_cross_the_edge() {
        let c = city();
        let ways = Places::derive(&c).ways_in;
        assert!(!ways.is_empty());
        for p in &ways {
            assert!(on_the_edge(&c, *p), "{p:?}");
            assert!(crosses_the_edge(&c, *p), "{p:?}");
        }
        // a road two lanes wide laid along the top edge, where no road runs in, leads
        // nowhere in
        let mut along = c.clone();
        let w = c.size.w as i32;
        let clear = |x: i32| (2..4).all(|y| !carries_traffic(&c, Pos::new(x, y)));
        let xs: Vec<i32> = (1..w - 1).filter(|x| clear(*x)).collect();
        assert!(xs.len() >= 8, "room along the top");
        for &x in &xs {
            for y in 0..2 {
                along
                    .tile_mut(StratumKind::Street, Pos::new(x, y))
                    .unwrap()
                    .terrain = Terrain::Road;
            }
        }
        let ways = Places::derive(&along).ways_in;
        assert!(
            !ways.iter().any(|p| p.y == 0 && xs.contains(&p.x)),
            "a way in along the edge: {ways:?}"
        );
    }

    #[test]
    fn someone_with_no_home_comes_in_by_road_and_walks_back_out() {
        let c = city();
        let ways = Places::derive(&c).ways_in;
        let mut w = world(&c, 4 * 60);
        let day = day_workers(&w, 7 * 60, 10 * 60);
        assert!(day.len() >= 10, "{} day workers with no home", day.len());
        assert!(
            day.iter()
                .all(|i| w.people()[*i].whereabouts == Whereabouts::Away)
        );
        let mut came = vec![false; w.people().len()];
        for _ in 0..11 * 60 {
            a_minute(&mut w, &c);
            for &i in &day {
                if let Whereabouts::Travelling(t) = &w.people()[i].whereabouts
                    && t.depart == w.tick()
                    && ways.contains(&t.from)
                {
                    came[i] = true;
                    assert_eq!(w.pos_of(w.people()[i].id), Some(t.from), "seen at the edge");
                }
            }
        }
        // three in the afternoon, after any break: every one came in by a way in, and is
        // at work
        for &i in &day {
            let p = &w.people()[i];
            assert!(came[i], "{} came in by road", p.name);
            let Work::Building(b) = p.work else {
                unreachable!()
            };
            assert_eq!(p.whereabouts, Whereabouts::Inside(b), "{} at work", p.name);
        }
        // three in the morning: every one has walked back out
        let mut left = vec![false; w.people().len()];
        for _ in 0..12 * 60 {
            a_minute(&mut w, &c);
            for &i in &day {
                if let Whereabouts::Travelling(t) = &w.people()[i].whereabouts
                    && t.depart == w.tick()
                    && let Some(end) = t.legs.last().and_then(|l| l.path.last())
                    && ways.contains(end)
                {
                    left[i] = true;
                }
            }
        }
        for &i in &day {
            let p = &w.people()[i];
            assert!(left[i], "{} walked out by road", p.name);
            assert_eq!(p.whereabouts, Whereabouts::Away, "{} gone", p.name);
            assert_eq!(w.pos_of(p.id), None);
        }
    }

    #[test]
    fn a_commuter_sets_off_as_early_as_the_walk_in_takes() {
        let c = city();
        let places = Places::derive(&c);
        let mut w = world(&c, 4 * 60);
        let day = day_workers(&w, 7 * 60, 10 * 60);
        let mut set_off = vec![None; w.people().len()];
        let mut there = vec![None; w.people().len()];
        for m in 1..=9 * 60u16 {
            a_minute(&mut w, &c);
            for &i in &day {
                match &w.people()[i].whereabouts {
                    Whereabouts::Travelling(t) if t.depart == w.tick() => {
                        set_off[i].get_or_insert(4 * 60 + m);
                    }
                    Whereabouts::Inside(_) => {
                        there[i].get_or_insert(4 * 60 + m);
                    }
                    _ => {}
                }
            }
        }
        for &i in &day {
            let p = &w.people()[i];
            let Work::Building(b) = p.work else {
                unreachable!()
            };
            let way = w.comes_in_by(p.id).unwrap();
            let door = places.door_from(b, way).unwrap();
            let work = p.schedule.iter().find(|s| s.goal == Goal::Work).unwrap().at;
            assert_eq!(
                set_off[i],
                Some(work - walk_minutes(way, door)),
                "{} sets off as early as the walk takes",
                p.name
            );
            assert!(
                there[i].is_some_and(|m| m <= p.shift.0),
                "{} at work by {}: {:?}",
                p.name,
                p.shift.0,
                there[i]
            );
        }
    }

    #[test]
    fn someone_who_lives_in_town_has_their_drink_in_town() {
        // no bar in the city: those with a home in it who'd go for a drink after work
        // have it at what it has (a noodle bar, the Strip) and never go out of town for
        // it; with nowhere at all for a drink, they stay home
        let mut c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap();
        for b in &mut c.buildings {
            if b.kind == BuildingKind::Bar {
                b.kind = BuildingKind::Warehouse;
            }
        }
        let mut dry = c.clone();
        for b in &mut dry.buildings {
            if b.kind == BuildingKind::NoodleBar {
                b.kind = BuildingKind::Warehouse;
            }
        }
        for d in &mut dry.districts {
            if matches!(d.theme, Theme::NeonStrip | Theme::NightMarket) {
                d.theme = Theme::OldTown;
            }
        }
        assert!(!Places::derive(&c).drinks.is_empty());
        assert!(Places::derive(&dry).drinks.is_empty());
        for (c, somewhere) in [(&c, true), (&dry, false)] {
            let mut w = world(c, 4 * 60);
            let drinkers: Vec<usize> = w
                .people()
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.home.is_some()
                        && p.schedule
                            .iter()
                            .any(|s| s.goal == Goal::Leisure(crate::Leisure::Bar))
                })
                .map(|(i, _)| i)
                .collect();
            assert!(!drinkers.is_empty());
            let mut had_one = vec![false; w.people().len()];
            for _ in 0..24 * 60 {
                a_minute(&mut w, c);
                for &i in &drinkers {
                    let p = &w.people()[i];
                    assert_ne!(p.whereabouts, Whereabouts::Away, "{} left town", p.name);
                    // in for a drink: inside somewhere neither home nor work
                    if let Whereabouts::Inside(b) = p.whereabouts
                        && matches!(p.goal, Goal::Leisure(_))
                        && Some(b) != p.home
                        && p.work != Work::Building(b)
                    {
                        had_one[i] = true;
                    }
                }
            }
            let drank = drinkers.iter().filter(|i| had_one[**i]).count();
            if somewhere {
                assert!(
                    drank * 2 > drinkers.len(),
                    "{drank} of {} had a drink in town",
                    drinkers.len()
                );
            } else {
                assert_eq!(drank, 0, "a drink with nowhere to have one");
            }
        }
    }

    #[test]
    fn a_map_no_road_leaves_keeps_its_people_on_the_street() {
        // every road off the map cut: nobody goes off it, as before there were ways in
        let mut c = city();
        let edge: Vec<Pos> = edge(&c).collect();
        for p in edge {
            if carries_traffic(&c, p) {
                c.tile_mut(StratumKind::Street, p).unwrap().terrain = Terrain::Water;
            }
        }
        assert!(Places::derive(&c).ways_in.is_empty());
        let mut w = world(&c, 4 * 60);
        for _ in 0..24 * 60 {
            a_minute(&mut w, &c);
            assert!(
                w.people()
                    .iter()
                    .all(|p| p.whereabouts != Whereabouts::Away)
            );
        }
        let spot = w
            .people()
            .iter()
            .find(|p| p.home.is_none() && matches!(p.work, Work::Spot(_)))
            .expect("someone with no home works a spot");
        let card = w.card(spot.id, &c);
        assert_eq!(card.lives.as_deref(), Some("the street"));
    }

    #[test]
    fn someone_off_the_map_comes_back_from_a_save() {
        let c = city();
        let people = People::load().unwrap();
        let mut w = world(&c, 6 * 60);
        for _ in 0..90 {
            a_minute(&mut w, &c);
        }
        let ways = Places::derive(&c).ways_in;
        let away = w
            .people()
            .iter()
            .filter(|p| p.whereabouts == Whereabouts::Away)
            .count();
        let walking = w
            .people()
            .iter()
            .filter(
                |p| matches!(&p.whereabouts, Whereabouts::Travelling(t) if ways.contains(&t.from)),
            )
            .count();
        assert!(away > 0 && walking > 0, "{away} away, {walking} walking in");
        let bytes = rmp_serde::to_vec_named(&w).unwrap();
        let mut back: World = rmp_serde::from_slice(&bytes).unwrap();
        back.rebuild(&c, &people);
        for _ in 0..12 * 60 {
            a_minute(&mut w, &c);
            a_minute(&mut back, &c);
        }
        for (a, b) in w.people().iter().zip(back.people()) {
            assert_eq!(a.whereabouts, b.whereabouts, "{}", a.name);
        }
    }

    #[test]
    fn a_spot_worker_with_no_home_keeps_to_their_spot() {
        // off shift as on it: never off the map, and at noon on their spot
        let c = city();
        let mut w = world(&c, 4 * 60);
        let spot: Vec<usize> = w
            .people()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.home.is_none() && matches!(p.work, Work::Spot(_)))
            .map(|(i, _)| i)
            .collect();
        assert!(spot.len() >= 5, "{} spot workers with no home", spot.len());
        for _ in 0..24 * 60 {
            a_minute(&mut w, &c);
            for &i in &spot {
                let p = &w.people()[i];
                assert_ne!(p.whereabouts, Whereabouts::Away, "{} left town", p.name);
                if w.minute_of_day() == 12 * 60 {
                    let Work::Spot(at) = p.work else {
                        unreachable!()
                    };
                    assert!(
                        w.pos_of(p.id).is_some_and(|q| q.manhattan(at) <= 1),
                        "{} on their spot at noon: {:?}",
                        p.name,
                        p.whereabouts
                    );
                }
            }
        }
    }

    #[test]
    fn someone_kept_in_town_never_goes_off_the_map() {
        // a day worker with no home, kept in town (the story has a part for them): in at
        // work by day, and never out of town after
        let c = city();
        let mut w = world(&c, 12 * 60);
        let kept: Vec<usize> = day_workers(&w, 7 * 60, 10 * 60)
            .into_iter()
            .take(5)
            .collect();
        assert_eq!(kept.len(), 5);
        for &i in &kept {
            let id = w.people()[i].id;
            w.keep_in_town(id);
            assert_eq!(w.card(id, &c).lives.as_deref(), Some("the street"));
        }
        for _ in 0..24 * 60 {
            a_minute(&mut w, &c);
            for &i in &kept {
                let p = &w.people()[i];
                assert_ne!(p.whereabouts, Whereabouts::Away, "{} left town", p.name);
            }
        }
    }

    #[test]
    fn someone_kept_in_town_while_away_comes_back() {
        // day workers with no home, out of town at four in the morning when the story
        // casts them: back in the next minute the sim runs (walking in, or at work), and
        // in town from then on
        let c = city();
        let mut w = world(&c, 4 * 60);
        let kept: Vec<usize> = day_workers(&w, 7 * 60, 10 * 60)
            .into_iter()
            .take(5)
            .collect();
        assert_eq!(kept.len(), 5);
        for &i in &kept {
            assert_eq!(w.people()[i].whereabouts, Whereabouts::Away);
            w.keep_in_town(w.people()[i].id);
        }
        a_minute(&mut w, &c);
        for &i in &kept {
            let p = &w.people()[i];
            let Work::Building(b) = p.work else {
                unreachable!()
            };
            let back = match &p.whereabouts {
                Whereabouts::Travelling(t) => t.dest == Dest::Building(b),
                at => *at == Whereabouts::Inside(b),
            };
            assert!(back, "{} back for work: {:?}", p.name, p.whereabouts);
        }
        for _ in 0..24 * 60 {
            a_minute(&mut w, &c);
            for &i in &kept {
                let p = &w.people()[i];
                assert_ne!(p.whereabouts, Whereabouts::Away, "{} left town", p.name);
            }
        }
    }

    #[test]
    fn someone_whose_way_in_never_walks_to_work_stays_in_town() {
        // many of those who work in seed 0's Northmarch with no home there have no way
        // in that walks to their work: they don't commute, but stay at or inside their
        // work all day, as before anyone commuted
        let (_, params) = settlements_of(0)
            .into_iter()
            .find(|(n, _)| n == "Northmarch")
            .unwrap();
        let c = Generator::run(params).unwrap();
        let places = Places::derive(&c);
        let walks: Vec<(Pos, citygen::Grid<bool>)> = places
            .ways_in
            .iter()
            .map(|way| (*way, walk_from(&c, *way)))
            .collect();
        let walks_to = |b| {
            let doors = &places.doors[&b];
            walks.iter().any(|(way, walk)| {
                let into = doors.iter().min_by_key(|d| d.manhattan(*way)).unwrap();
                walk[*into] && walk[doors[0]]
            })
        };
        let mut w = world(&c, 4 * 60);
        let stay: Vec<usize> = w
            .people()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.home.is_none() && p.profession != "fixer")
            .filter(|(_, p)| matches!(p.work, Work::Building(b) if !walks_to(b)))
            .map(|(i, _)| i)
            .collect();
        assert!(stay.len() >= 50, "{} stay in town", stay.len());
        for m in 0..24 * 60 {
            for &i in &stay {
                let p = &w.people()[i];
                assert_ne!(
                    p.whereabouts,
                    Whereabouts::Away,
                    "{} out of town at {m}",
                    p.name
                );
            }
            a_minute(&mut w, &c);
        }
    }

    /// The cities and towns of world `seed` as a game builds them on their sites
    /// (`screens::build_home`, `outside::build_params`): the home city on the world's own
    /// seed, the rest each on a seed of its own. Each named as the world names it.
    fn settlements_of(seed: u64) -> Vec<(String, GenParams)> {
        use citygen::overworld::{Overworld, WorldParams, WorldSize};
        use citygen::{sited, sited_town};
        let d = WorldParams::default();
        let citygen::Size { w, h } = d.size.size();
        let size = WorldSize::Custom { w, h };
        let world = Overworld::generate(WorldParams { seed, size, ..d }).unwrap();
        let base = GenParams {
            size: SizePreset::Large,
            ..Default::default()
        };
        let home = world.cities.iter().position(|c| c.home).unwrap_or(0) as u8;
        let mut out = vec![(
            world.settlement_name(home, None),
            sited(&world, home, base.clone().with_seed(seed)),
        )];
        for (i, c) in world.cities.iter().enumerate() {
            let i = i as u8;
            let own = |t: Option<u8>| {
                let at = (u64::from(i) << 8) | t.map_or(0, |t| u64::from(t) + 1);
                base.clone()
                    .with_seed(hash64(world.seed ^ 0xC17E, at) % 1_000_000_007)
            };
            if i != home {
                out.push((c.name.clone(), sited(&world, i, own(None))));
            }
            for t in 0..c.towns.len() as u8 {
                if let Some(p) = sited_town(&world, i, t, own(Some(t))) {
                    out.push((format!("{}'s town {t}", c.name), p));
                }
            }
        }
        out
    }

    /// The cells of `c` a walk from `from` reaches, stepping as a walker does.
    fn walk_from(c: &City, from: Pos) -> citygen::Grid<bool> {
        let mut seen = citygen::Grid::filled(c.size, false);
        let mut queue = std::collections::VecDeque::from([from]);
        seen.set(from, true);
        while let Some(p) = queue.pop_front() {
            for n in p.neighbors4() {
                if seen.get(n) == Some(&false) && walk_cost(c, n, n).is_some() {
                    seen.set(n, true);
                    queue.push_back(n);
                }
            }
        }
        seen
    }

    /// How many of `c`'s people come in to work a building from off the map, and those
    /// of them whose way in doesn't walk to the door they go in by or come out of.
    fn cut_off(c: &City) -> (usize, Vec<String>) {
        let w = world(c, 4 * 60);
        let places = Places::derive(c);
        let mut walks: HashMap<Pos, citygen::Grid<bool>> = HashMap::new();
        let (mut commuters, mut cut) = (0, Vec::new());
        for p in w.people() {
            let (Some(way), Work::Building(b)) = (w.comes_in_by(p.id), p.work) else {
                continue;
            };
            commuters += 1;
            let walk = walks.entry(way).or_insert_with(|| walk_from(c, way));
            let doors = places.doors.get(&b).map_or(&[][..], |d| &d[..]);
            let into = doors.iter().min_by_key(|d| d.manhattan(way));
            let out_of = doors.first();
            let (walks_in, walks_out) = (
                into.is_some_and(|d| walk[*d]),
                out_of.is_some_and(|d| walk[*d]),
            );
            if !walks_in || !walks_out {
                let no = if walks_in { "out" } else { "in" };
                cut.push(format!(
                    "{} ({}) by {way:?}: no walk {no}",
                    p.name, p.profession
                ));
            }
        }
        (commuters, cut)
    }

    #[test]
    fn a_commuters_way_in_walks_to_work() {
        // the cities and towns where the review found people coming in by a way that
        // never walks to their work (and 21's Ironfall, as 21's Ironburg has nobody
        // commuting since its districts are its colours): every one now comes by one
        // that does, or stays in town
        for (seed, name) in [
            (0, "Portgate"),
            (2, "Porthaven"),
            (10, "Northspire"),
            (21, "Ironfall"),
            (23, "Staticwick"),
        ] {
            let (_, params) = settlements_of(seed)
                .into_iter()
                .find(|(n, _)| n == name)
                .unwrap();
            let c = Generator::run(params).unwrap();
            let (commuters, cut) = cut_off(&c);
            assert!(commuters > 0, "seed {seed} {name}: nobody commutes");
            assert!(
                cut.is_empty(),
                "seed {seed} {name}: {} of {commuters} cut off: {:?}",
                cut.len(),
                &cut[..cut.len().min(5)]
            );
        }
    }

    #[test]
    #[ignore = "every city and town of 41 worlds: minutes in the debug profile"]
    fn every_commuters_way_in_walks_to_work() {
        let (mut all, mut cut_all) = (0, 0);
        for seed in 0..=40 {
            for (name, params) in settlements_of(seed) {
                let Ok(c) = Generator::run(params) else {
                    continue;
                };
                let (commuters, cut) = cut_off(&c);
                let out = cut.iter().filter(|s| s.ends_with("no walk out")).count();
                println!(
                    "seed {seed} {name}: {commuters} commuters, {} cut off ({out} only on the way out)",
                    cut.len()
                );
                all += commuters;
                cut_all += cut.len();
            }
        }
        println!("{cut_all} of {all} commuters cut off");
        assert_eq!(cut_all, 0);
    }

    #[test]
    fn a_commuter_says_they_live_out_of_town() {
        let c = city();
        let mut w = world(&c, 4 * 60);
        let i = day_workers(&w, 7 * 60, 10 * 60)[0];
        let id = w.people()[i].id;
        let card = w.card(id, &c);
        assert_eq!(card.doing, "out of town");
        assert_eq!(card.lives.as_deref(), Some("out of town"));
        let on_the_way = |w: &World, goal: Goal| matches!(&w.people()[i].whereabouts, Whereabouts::Travelling(t) if t.goal == goal);
        for _ in 0..24 * 60 {
            if on_the_way(&w, Goal::Work) {
                break;
            }
            a_minute(&mut w, &c);
        }
        let card = w.card(id, &c);
        assert_eq!(card.doing, "heading to work");
        for _ in 0..24 * 60 {
            if on_the_way(&w, Goal::Home) {
                break;
            }
            a_minute(&mut w, &c);
        }
        let card = w.card(id, &c);
        assert_eq!(card.doing, "heading home");
        assert!(
            card.next.starts_with("arrives at the edge of town in "),
            "{}",
            card.next
        );
    }
}
