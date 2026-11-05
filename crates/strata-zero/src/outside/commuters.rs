//! People who work in a city with no home in it commute (`2026-11-05-streamed-world-
//! design.md`, "After the first play"): they live off the map, come in by a road where it
//! crosses the map's edge, walk to work and walk back out. Played on seed 3's world.

use crate::outside::Settlement;
use crate::outside::stream::tests::settlements;
use citygen::rng::hash64;
use citygen::{City, Pos, StratumKind, Terrain};
use sim::{Dest, Goal, Leisure, PersonId, Whereabouts, Work, World};
use std::collections::BTreeSet;

/// Seed 3's corporate neighbour: towers and offices, and homes for few who work there.
const CORPORATE: Settlement = Settlement {
    city: 1,
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
fn a_day(
    city: &City,
    minute: u16,
    minutes: u64,
    mut look: impl FnMut(&World),
) -> (World, Vec<Trip>) {
    let people = sim::People::load().unwrap();
    let mut w = World::seed(city, &people, &sim::SimParams::default(), minute, 0);
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
    let city = settlement(CORPORATE);
    let people = sim::People::load().unwrap();
    let seeded = World::seed(city, &people, &sim::SimParams::default(), FOUR, 0);
    let homes = seeded.people().iter().filter(|p| p.home.is_some()).count();
    assert!(
        homes * 4 < seeded.people().len(),
        "few work there with a home in it: {homes} of {}",
        seeded.people().len()
    );
    // those who work its buildings by day with no home in it
    let day: BTreeSet<PersonId> = seeded
        .people()
        .iter()
        .filter(|p| p.home.is_none() && matches!(p.work, Work::Building(_)))
        .filter(|p| (7 * 60..=11 * 60).contains(&p.shift_start))
        .map(|p| p.id)
        .collect();
    assert!(day.len() >= 40, "{} day workers", day.len());
    let (mut at_five, mut at_one) = (Vec::new(), Vec::new());
    let (w, trips) = a_day(city, FOUR, 24 * 60, |w| match w.minute_of_day() {
        300 => at_five = day.iter().map(|id| off_the_map(w, *id)).collect(),
        780 => {
            at_one = day
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
    assert!(at_one.iter().all(|at| *at), "at work at one");
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
        (760, 0x854a_e2f5_e865_f21f),
        "the home city's people with a home changed their day"
    );
    // towns where everyone has a home and every trip has a place in it: their whole day
    // as it was
    for (s, was) in [
        ((5, 0), (42, 0xaac3_6dfd_b8e6_2a07)),
        ((6, 0), (35, 0x2365_2568_85cf_091c)),
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
