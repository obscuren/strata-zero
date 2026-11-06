//! People who work in a city with no home in it commute (`2026-11-05-streamed-world-
//! design.md`, "After the first play"): they live off the map, come in by a road where it
//! crosses the map's edge, walk to work and walk back out. Played on seed 3's world.

use crate::outside::Settlement;
use crate::outside::stream::tests::settlements;
use citygen::rng::hash64;
use citygen::{City, Pos, StratumKind, Terrain};
use sim::{Dest, Goal, Leisure, PersonId, Whereabouts, Work, World};
use std::collections::BTreeSet;

/// Seed 3's neighbour of docks, the strip and a corp core: homes for under a third who
/// work there.
const CORPORATE: Settlement = Settlement {
    city: 2,
    town: None,
};

/// Seed 3's home city, seeded as home is (not housed): homes for under half who work
/// there, the rest coming in from off the map. Every other city has homes for half its
/// people at least.
const HOME: Settlement = Settlement {
    city: 0,
    town: None,
};

/// One trip of a day: who, the tick they set off, what for, where to, and its two ends.
#[derive(Clone, Debug, PartialEq)]
struct Trip {
    who: PersonId,
    depart: u64,
    goal: Goal,
    dest: Dest,
    from: Pos,
    to: Pos,
}

/// The day starts at four in the morning, before anyone sets off for a day's work.
const FOUR: u16 = 4 * 60;

/// Settlement `s` of seed 3's world, built as the game builds it.
fn settlement(s: Settlement) -> &'static City {
    &settlements().iter().find(|(t, _)| *t == s).unwrap().1
}

/// `city`'s people seeded at `minute` of the day (tick 0) and run on `minutes` a minute at
/// a time, every trip pathed as it's asked for; `look` sees the world after each minute.
/// Every trip set off, in order.
fn a_day(city: &City, minute: u16, minutes: u64, look: impl FnMut(&World)) -> (World, Vec<Trip>) {
    let people = sim::People::load().unwrap();
    let w = World::seed(city, &people, &sim::SimParams::default(), minute, 0);
    a_day_of(city, w, minutes, look)
}

/// `w`, `city`'s people seeded at tick 0, run on `minutes` a minute at a time, as
/// `a_day` runs them.
fn a_day_of(
    city: &City,
    mut w: World,
    minutes: u64,
    mut look: impl FnMut(&World),
) -> (World, Vec<Trip>) {
    let mut trips = Vec::new();
    for m in 1..=minutes {
        w.advance(city, m * sim::TICKS_PER_MINUTE);
        w.path_some(city, usize::MAX);
        for p in w.people() {
            if let Whereabouts::Travelling(t) = &p.whereabouts
                && t.depart == w.tick()
            {
                trips.push(Trip {
                    who: p.id,
                    depart: t.depart,
                    goal: t.goal,
                    dest: t.dest,
                    from: t.from,
                    to: t.legs.last().and_then(|l| l.path.last().copied()).unwrap(),
                });
            }
        }
        look(&w);
    }
    (w, trips)
}

/// The minute of the day a trip of a day begun at `FOUR` set off.
fn set_off(t: &Trip) -> u16 {
    ((FOUR as u64 + t.depart / sim::TICKS_PER_MINUTE) % 1440) as u16
}

/// A fingerprint of a list: any change to an item changes it.
fn digest<T: std::fmt::Debug>(items: &[T]) -> u64 {
    items.iter().fold(0, |h, t| {
        format!("{t:?}")
            .bytes()
            .fold(h, |h, b| hash64(h, u64::from(b)))
    })
}

/// Whether `p` is on the edge of `city`'s map where a road crosses it: road (or a bridge)
/// there and on the next three cells in from it.
fn a_road_across_the_edge(city: &City, p: Pos) -> bool {
    let (w, h) = (city.size.w as i32, city.size.h as i32);
    let road = |q: Pos| {
        city.tile(StratumKind::Street, q)
            .is_some_and(|t| matches!(t.terrain, Terrain::Road | Terrain::Bridge))
    };
    let (dx, dy) = match p {
        _ if p.x == 0 => (1, 0),
        _ if p.x == w - 1 => (-1, 0),
        _ if p.y == 0 => (0, 1),
        _ if p.y == h - 1 => (0, -1),
        _ => return false,
    };
    (0..4).all(|k| road(p.offset(dx * k, dy * k)))
}

/// Whether `p` lies on the edge of `city`'s map.
fn on_the_edge(city: &City, p: Pos) -> bool {
    let (w, h) = (city.size.w as i32, city.size.h as i32);
    p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1
}

/// Off the map: on no street, in no building and on no way.
fn off_the_map(w: &World, id: PersonId) -> bool {
    w.pos_of(id).is_none()
        && !matches!(
            w.people()[id.0 as usize].whereabouts,
            Whereabouts::Inside(_) | Whereabouts::Travelling(_)
        )
}

#[test]
fn a_city_without_homes_still_has_its_commute() {
    let city = settlement(HOME);
    let people = sim::People::load().unwrap();
    let seeded = World::seed(city, &people, &sim::SimParams::default(), FOUR, 0);
    let homes = seeded.people().iter().filter(|p| p.home.is_some()).count();
    assert!(
        homes * 2 < seeded.people().len(),
        "few work there with a home in it: {homes} of {}",
        seeded.people().len()
    );
    // those who work its buildings by day with no home in it, and a way in that walks
    // to their work (the rest stay in town)
    let day: BTreeSet<PersonId> = seeded
        .people()
        .iter()
        .filter(|p| p.home.is_none() && matches!(p.work, Work::Building(_)))
        .filter(|p| (7 * 60..=11 * 60).contains(&p.shift_start))
        .filter(|p| seeded.comes_in_by(p.id).is_some())
        .map(|p| p.id)
        .collect();
    assert!(day.len() >= 40, "{} day workers", day.len());
    let (mut at_five, mut at_three) = (Vec::new(), Vec::new());
    let (w, trips) = a_day(city, FOUR, 24 * 60, |w| match w.minute_of_day() {
        300 => at_five = day.iter().map(|id| off_the_map(w, *id)).collect(),
        // after any break at midday
        900 => {
            at_three = day
                .iter()
                .map(|id| {
                    let p = &w.people()[id.0 as usize];
                    matches!((p.work, &p.whereabouts),
                        (Work::Building(b), Whereabouts::Inside(i)) if b == *i)
                })
                .collect()
        }
        _ => {}
    });
    // before the morning nobody stands about at a door with nowhere to go: they're off
    // the map
    assert!(at_five.iter().all(|off| *off), "off the map at five");
    for id in &day {
        let p = &w.people()[id.0 as usize];
        let Work::Building(b) = p.work else {
            unreachable!()
        };
        let mine: Vec<&Trip> = trips.iter().filter(|t| t.who == *id).collect();
        // in by a road across the edge in the morning, to a door of their work
        let morning = mine
            .iter()
            .position(|t| {
                t.goal == Goal::Work
                    && a_road_across_the_edge(city, t.from)
                    && (5 * 60..11 * 60).contains(&set_off(t))
            })
            .unwrap_or_else(|| panic!("{} never comes in: {mine:?}", p.name));
        assert!(
            w.doors(b).contains(&mine[morning].to),
            "{} walks to work",
            p.name
        );
        // and back out by a road across the edge later in the day
        assert!(
            mine[morning + 1..]
                .iter()
                .any(|t| a_road_across_the_edge(city, t.to)),
            "{} never leaves: {mine:?}",
            p.name
        );
        assert!(off_the_map(&w, *id), "{} is gone by the night", p.name);
    }
    assert!(at_three.iter().all(|at| *at), "at work at three");
    // the day has trips: every day worker in and out
    assert!(
        trips.len() >= 2 * day.len(),
        "{} trips in a day",
        trips.len()
    );
}

#[test]
fn commuters_come_in_where_a_road_crosses_the_edge() {
    // the corporate neighbour, and a city where nobody has a home
    let all_out = Settlement {
        city: 3,
        town: None,
    };
    for s in [CORPORATE, all_out] {
        let city = settlement(s);
        let mut off: BTreeSet<PersonId> = BTreeSet::new();
        let (mut came, mut wrong) = (0, Vec::new());
        let (w, trips) = a_day(city, FOUR, 24 * 60, |w| {
            for p in w.people() {
                // set off this minute from off the map: on a road across the edge, and
                // seen there
                if let Whereabouts::Travelling(t) = &p.whereabouts
                    && t.depart == w.tick()
                    && off.contains(&p.id)
                {
                    came += 1;
                    if !a_road_across_the_edge(city, t.from) || w.pos_of(p.id) != Some(t.from) {
                        wrong.push((p.name.clone(), t.from));
                    }
                }
            }
            off = w
                .people()
                .iter()
                .filter(|p| off_the_map(w, p.id))
                .map(|p| p.id)
                .collect();
        });
        assert!(came > 0, "{s:?}: nobody comes in");
        assert!(wrong.is_empty(), "{s:?}: came in elsewhere: {wrong:?}");
        // and whoever walks to the edge leaves by a road across it
        let out: Vec<&Trip> = trips.iter().filter(|t| on_the_edge(city, t.to)).collect();
        assert!(!out.is_empty(), "{s:?}: nobody leaves");
        for t in out {
            assert!(
                a_road_across_the_edge(city, t.to),
                "{s:?}: {} leaves at {:?}",
                w.people()[t.who.0 as usize].name,
                t.to
            );
        }
    }
}

/// The trips of people with a home in the city that no die decides, as who, when, what
/// for and where to. The dice (a beat on the Strip and how a date there goes, a courier's
/// next drop, the step a door's muscle takes) draw from one stream for the whole city,
/// and those who come in from off the map draw from it too; the rest of a day is the
/// schedule's. So: all but couriers' trips and the days of those who go to the Strip.
fn of_the_schedule(w: &World, trips: &[Trip]) -> Vec<(PersonId, u64, Goal, Dest)> {
    let strip: BTreeSet<PersonId> = trips
        .iter()
        .filter(|t| t.goal == Goal::Leisure(Leisure::Strip))
        .map(|t| t.who)
        .collect();
    trips
        .iter()
        .filter(|t| {
            let p = &w.people()[t.who.0 as usize];
            p.home.is_some() && p.work != Work::Roaming && !strip.contains(&t.who)
        })
        .map(|t| (t.who, t.depart, t.goal, t.dest))
        .collect()
}

#[test]
fn a_city_with_homes_keeps_its_day() {
    // seed 3's home city: its people with a home in it keep their day as it was before
    // anyone commuted (pinned at 5e2230b), all but what the dice decide
    let (home, city) = &settlements()[0];
    assert_eq!(home.town, None);
    let (w, trips) = a_day(city, FOUR, 24 * 60, |_| {});
    let kept = of_the_schedule(&w, &trips);
    assert_eq!(
        (kept.len(), digest(&kept)),
        (535, 0xd8e5_b189_5c1c_dd26),
        "the home city's people with a home changed their day"
    );
    // towns where everyone has a home and every trip has a place in it: their whole day
    // as it was
    for (s, was) in [
        ((5, 0), (42, 0xaac3_6dfd_b8e6_2a07)),
        ((6, 0), (12, 0x4399_9970_2b31_d156)),
        ((6, 1), (18, 0x8fea_6f3e_57f4_b188)),
    ] {
        let town = settlement(Settlement {
            city: s.0,
            town: Some(s.1),
        });
        let (_, trips) = a_day(town, FOUR, 24 * 60, |_| {});
        assert_eq!(
            (trips.len(), digest(&trips)),
            was,
            "town {s:?} changed its day"
        );
    }
}

/// `city`'s people as the game seeds them when you come into it or see it next door
/// (`outside::seed_people`), at `FOUR` on the clock's first day.
fn come_into(city: &City) -> World {
    crate::outside::seed_people(city, &sim::People::load().unwrap(), 0, FOUR)
}

/// The ticks `w`'s person `p` drives on a trip set off this minute: none on foot.
fn drives(w: &World, p: &sim::Person) -> u64 {
    match &p.whereabouts {
        Whereabouts::Travelling(t) if t.depart == w.tick() => t
            .legs
            .iter()
            .filter(|l| l.car.is_some())
            .map(|l| l.path.len().saturating_sub(1) as u64 * sim::DRIVE_TICKS)
            .sum(),
        _ => 0,
    }
}

#[test]
fn a_city_you_come_into_lives_in_town() {
    // seed 3's corporate neighbour as you find it, with flats for few who work there:
    // those who work there live there all the same, so its people live their day
    // in town as the home city's do: home by night, and nobody walks out of town, not
    // even for a drink it hasn't got
    let city = settlement(CORPORATE);
    let w = come_into(city);
    let n = w.people().len();
    let homes = w.people().iter().filter(|p| p.home.is_some()).count();
    assert_eq!(homes, n, "{homes} of {n} have a home in town");
    let (mut away, mut at_home) = (Vec::new(), 0);
    let (_, trips) = a_day_of(city, w, 24 * 60, |w| {
        let off = w
            .people()
            .iter()
            .filter(|p| p.whereabouts == Whereabouts::Away)
            .count();
        if off > 0 && w.minute_of_day() % 60 == 0 {
            away.push((w.minute_of_day() / 60, off));
        }
        if w.minute_of_day() == 3 * 60 {
            at_home = w
                .people()
                .iter()
                .filter(|p| {
                    p.home
                        .is_some_and(|h| p.whereabouts == Whereabouts::Inside(h))
                })
                .count();
        }
    });
    assert!(away.is_empty(), "off the map, by the hour: {away:?}");
    let out: Vec<&Trip> = trips
        .iter()
        .filter(|t| matches!(t.dest, Dest::Away(_)))
        .collect();
    assert!(out.is_empty(), "out of town: {out:?}");
    assert!(at_home * 2 > n, "{at_home} of {n} at home at three");
}

#[test]
fn every_city_you_come_into_has_its_traffic() {
    // seed 3's other cities with housing as you find them: a car for at least one in ten
    // who live there, and the cars driven, a drive a day for every three at least. The
    // bound is each city's own, not the home city's: most of home's driving is couriers
    // with cars on their rounds, and its long commutes come from its big map and its
    // housing far from its work
    let (home, _) = &settlements()[0];
    let mut short = Vec::new();
    for (s, city) in settlements() {
        let there = come_into(city);
        if s == home || s.town.is_some() || there.cars().is_empty() {
            continue;
        }
        let lives = there.people().iter().filter(|p| p.home.is_some()).count();
        let cars = there.cars().len();
        let mut driven = 0;
        a_day_of(city, there, 24 * 60, |w| {
            driven += w.people().iter().filter(|p| drives(w, p) > 0).count();
        });
        if cars * 10 < lives || driven * 3 < cars {
            short.push(format!(
                "{s:?}: {cars} cars for {lives} who live there, {driven} drives a day"
            ));
        }
    }
    assert!(short.is_empty(), "short of traffic: {short:#?}");
}

/// New games' home cities for seeds 1 to 8, as a new game builds them: built once.
fn new_homes() -> &'static [(u64, City)] {
    static HOMES: std::sync::OnceLock<Vec<(u64, City)>> = std::sync::OnceLock::new();
    HOMES.get_or_init(|| {
        (1..=8)
            .map(|s| {
                let params = crate::app::AppState::new().params.with_seed(s);
                (s, crate::screens::build_home(params).unwrap())
            })
            .collect()
    })
}

/// A new game in seed `seed`'s home city: moved in at the hour a new game opens, the
/// story begun (its cast picked).
fn new_game(seed: u64) -> crate::app::AppState {
    let mut st = crate::app::AppState::new();
    let city = new_homes()
        .iter()
        .find(|(s, _)| *s == seed)
        .unwrap()
        .1
        .clone();
    st.enter_city(city);
    crate::missions::begin(&mut st);
    st
}

/// `st`'s people a minute on, every walk asked for found.
fn a_minute(st: &mut crate::app::AppState) {
    let city = st.city.as_ref().unwrap();
    let w = st.world.as_mut().unwrap();
    let t = w.tick() + sim::TICKS_PER_MINUTE;
    w.advance(city, t);
    w.path_some(city, usize::MAX);
}

#[test]
fn the_story_fixers_keep_their_corner() {
    // Coz and Mezz have no home in these cities: by day as by night they're at their
    // corner, where the story and the jobs send you
    let mut missed = Vec::new();
    for seed in [1, 7, 8] {
        let mut st = new_game(seed);
        let fixers: Vec<(&str, PersonId)> = ["coz", "mezz"]
            .into_iter()
            .map(|r| (r, st.missions.cast[r]))
            .collect();
        for (role, id) in &fixers {
            let p = &st.world.as_ref().unwrap().people()[id.0 as usize];
            assert!(p.home.is_none(), "seed {seed}: {role} has a home");
        }
        for _ in 0..25 * 60 {
            a_minute(&mut st);
            let w = st.world.as_ref().unwrap();
            let m = w.minute_of_day();
            if ![8 * 60, 12 * 60, 16 * 60, 22 * 60].contains(&m) {
                continue;
            }
            for (role, id) in &fixers {
                let spot = w.spot_of(*id).unwrap();
                if !w.pos_of(*id).is_some_and(|at| at.manhattan(spot) <= 1) {
                    missed.push((
                        seed,
                        *role,
                        m / 60,
                        w.people()[id.0 as usize].whereabouts.clone(),
                    ));
                }
            }
        }
    }
    assert!(missed.is_empty(), "not at their corner: {missed:?}");
}

#[test]
fn every_fixer_of_a_new_game_can_be_found_all_day() {
    // on the street or in a building, never off the map, from six in the morning to
    // eleven at night
    let mut missed = Vec::new();
    for seed in 1..=8 {
        let mut st = new_game(seed);
        let fixers: Vec<PersonId> = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| p.profession == "fixer")
            .map(|p| p.id)
            .collect();
        assert!(!fixers.is_empty(), "seed {seed}");
        for _ in 0..25 * 60 {
            a_minute(&mut st);
            let w = st.world.as_ref().unwrap();
            let m = w.minute_of_day();
            if !m.is_multiple_of(60) || !(6 * 60..=23 * 60).contains(&m) {
                continue;
            }
            for id in &fixers {
                let p = &w.people()[id.0 as usize];
                if w.pos_of(*id).is_none() && !matches!(p.whereabouts, Whereabouts::Inside(_)) {
                    missed.push((seed, p.name.clone(), m / 60));
                }
            }
        }
    }
    assert!(missed.is_empty(), "nowhere to be found: {missed:?}");
}

/// Those who work `city`'s buildings by day with no home in it, as seeded at four.
fn day_commuters(w: &World) -> Vec<(PersonId, citygen::BuildingId)> {
    w.people()
        .iter()
        .filter(|p| p.home.is_none() && (7 * 60..=11 * 60).contains(&p.shift_start))
        .filter_map(|p| match p.work {
            Work::Building(b) => Some((p.id, b)),
            _ => None,
        })
        .collect()
}

#[test]
fn commuters_take_lunch() {
    // a third or so of those who come in to work step out at midday, to something to eat
    // or a spot along the street, and are back at work after
    let city = settlement(HOME);
    let people = sim::People::load().unwrap();
    let day = day_commuters(&World::seed(
        city,
        &people,
        &sim::SimParams::default(),
        FOUR,
        0,
    ));
    let (mut out, mut back) = (Vec::new(), 0);
    a_day(city, FOUR, (15 * 60 - FOUR) as u64, |w| {
        let m = w.minute_of_day();
        for &(id, b) in &day {
            let p = &w.people()[id.0 as usize];
            if m == 12 * 60 + 45
                && p.whereabouts != Whereabouts::Inside(b)
                && p.whereabouts != Whereabouts::Away
            {
                out.push(id);
            }
            if m == 15 * 60 && p.whereabouts == Whereabouts::Inside(b) {
                back += 1;
            }
        }
    });
    assert!(
        out.len() >= 5 && out.len() * 2 <= day.len(),
        "{} of {} out at a quarter to one",
        out.len(),
        day.len()
    );
    assert_eq!(back, day.len(), "all back at work by three");
}

#[test]
fn some_commuters_stay_for_a_drink() {
    // a third or so of those who come in to work stay on in town after, at a bar, and
    // walk out late; by two in the morning they've all gone
    let (s, city) = [
        CORPORATE,
        Settlement {
            city: 2,
            town: None,
        },
    ]
    .into_iter()
    .map(|s| (s, settlement(s)))
    .find(|(_, c)| {
        c.buildings
            .iter()
            .any(|b| b.kind == citygen::world::BuildingKind::Bar)
    })
    .expect("a neighbour with a bar");
    let people = sim::People::load().unwrap();
    // those who work its buildings with no home in it and knock off by day
    let day: Vec<(PersonId, citygen::BuildingId)> =
        World::seed(city, &people, &sim::SimParams::default(), FOUR, 0)
            .people()
            .iter()
            .filter(|p| p.home.is_none() && (14 * 60..21 * 60).contains(&p.shift.1))
            .filter_map(|p| match p.work {
                Work::Building(b) => Some((p.id, b)),
                _ => None,
            })
            .collect();
    let (mut stayed, mut at_work, mut left) = (0, 0, 0);
    a_day(city, FOUR, (26 * 60 - FOUR) as u64, |w| {
        let m = w.minute_of_day();
        for &(id, b) in &day {
            let p = &w.people()[id.0 as usize];
            if m == 22 * 60 && p.whereabouts != Whereabouts::Away {
                stayed += 1;
                if p.whereabouts == Whereabouts::Inside(b) {
                    at_work += 1;
                }
            }
            if m == 2 * 60 && p.whereabouts == Whereabouts::Away {
                left += 1;
            }
        }
    });
    assert!(
        stayed >= 3 && stayed * 2 <= day.len(),
        "{s:?}: {stayed} of {} still in town at ten",
        day.len()
    );
    assert_eq!(at_work, 0, "{s:?}: in town for a drink, not at work");
    assert_eq!(left, day.len(), "{s:?}: all gone by two");
}

#[test]
fn the_cast_stays_in_town_after_a_reopen() {
    // a part of the story's who works a building with no home in the city never goes out
    // of town, in the run as begun and in it saved and reopened (the sim doesn't save the
    // mark: the cast marks it again)
    let (seed, role) = (1..=8)
        .flat_map(|s| ["marlo", "odile"].map(|r| (s, r)))
        .find(|&(s, r)| {
            let st = new_game(s);
            let id = st.missions.cast[r];
            let p = &st.world.as_ref().unwrap().people()[id.0 as usize];
            p.home.is_none() && matches!(p.work, Work::Building(_))
        })
        .expect("a cast part with no home in a new game's city");
    let st = new_game(seed);
    let id = st.missions.cast[role];
    let dir = crate::runs::testutil::Dir::new("cast");
    let path = crate::runs::file::write(&dir.0, &crate::runs::testutil::running(st)).unwrap();
    let mut back = crate::app::AppState::new();
    let (_, s, lost) = crate::runs::file::read(&path, &back.book).unwrap();
    crate::runs::snapshot::restore(&mut back, s);
    crate::runs::snapshot::after_restore(&mut back, lost);
    let mut fresh = new_game(seed);
    for st in [&mut fresh, &mut back] {
        for _ in 0..24 * 60 {
            a_minute(st);
            let p = &st.world.as_ref().unwrap().people()[id.0 as usize];
            assert_ne!(
                p.whereabouts,
                Whereabouts::Away,
                "seed {seed}: {role} went out of town"
            );
        }
    }
}
