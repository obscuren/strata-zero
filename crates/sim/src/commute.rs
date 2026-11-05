//! Commuting: someone who works in the city with no home in it lives off the map. They
//! come in by a road where it crosses the map's edge, walk to work, and walk back out the
//! same way; so does anyone whose trip ends at no place in the city (a bar the city
//! hasn't got). Off the map they are `Whereabouts::Away`, drawn nowhere.

use crate::paths::walk_cost;
use crate::people::PersonId;
use citygen::pathing::bfs_nearest;
use citygen::rng::hash64;
use citygen::world::{City, StratumKind};
use citygen::{Pos, Terrain};
use std::collections::HashSet;

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

/// Where people come in from off the map: the cells of the map's edge a road crosses
/// (`crosses_the_edge`) from which a walk reaches a street door. In order round the
/// edge; none for a map no road leaves.
pub fn ways_in(city: &City, doors: &HashSet<Pos>) -> Vec<Pos> {
    let reach = (city.size.w + city.size.h) as usize * 2;
    let mut ways: Vec<Pos> = Vec::new();
    for p in edge(city) {
        if !crosses_the_edge(city, p) || ways.contains(&p) {
            continue;
        }
        // the lane beside one that leads in leads in too
        let beside = ways.iter().any(|w| w.manhattan(p) == 1);
        let leads_in = beside
            || bfs_nearest(
                city.size,
                p,
                |q| walk_cost(city, q, q).is_some(),
                |q| doors.contains(&q),
                reach,
            )
            .is_some();
        if leads_in {
            ways.push(p);
        }
    }
    ways
}

/// The way in `who` comes and goes by, the same every day: none when the map has none.
pub fn way_in_of(ways: &[Pos], who: PersonId) -> Option<Pos> {
    if ways.is_empty() {
        return None;
    }
    let i = hash64(WAY, u64::from(who.0)) % ways.len() as u64;
    Some(ways[i as usize])
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
    use crate::people::{Goal, SimParams, Whereabouts, Work};
    use crate::places::Places;
    use crate::world::{TICKS_PER_MINUTE, World};
    use citygen::world::BuildingKind;
    use citygen::{GenParams, Generator, SizePreset};

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
        for _ in 0..9 * 60 {
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
        // one in the afternoon: every one came in by a way in, and is at work
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
        for _ in 0..14 * 60 {
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
            let way = way_in_of(&places.ways_in, p.id).unwrap();
            let door = *places.doors[&b]
                .iter()
                .min_by_key(|d| d.manhattan(way))
                .unwrap();
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
    fn a_trip_with_no_place_in_the_city_goes_out_of_town_and_back() {
        // no bar in the city: those who go for a drink after work go out of town for it,
        // and come home from there
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
        let mut w = world(&c, 4 * 60);
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
        let (mut out, mut back) = (vec![false; w.people().len()], vec![false; w.people().len()]);
        for _ in 0..24 * 60 {
            a_minute(&mut w, &c);
            for &i in &drinkers {
                let p = &w.people()[i];
                if p.whereabouts == Whereabouts::Away
                    && p.goal == Goal::Leisure(crate::Leisure::Bar)
                {
                    out[i] = true;
                }
                if out[i] && p.whereabouts == Whereabouts::Inside(p.home.unwrap()) {
                    back[i] = true;
                }
            }
        }
        for &i in &drinkers {
            let p = &w.people()[i];
            assert!(out[i], "{} went out of town for a drink", p.name);
            assert!(back[i], "{} came home", p.name);
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
