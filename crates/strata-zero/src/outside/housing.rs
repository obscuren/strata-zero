//! Who works in a city other than home lives in it (`sim::SimParams::housed`), where
//! they can walk to work, so every city and town of the world needs homes on its
//! streets: built as the game builds them and seeded as you find them, nine in ten of
//! its people have a home (all but two in a town of a few), and one of 50 or more has
//! its people in two buildings or more and beds for half of them at least
//! (`citygen::generate::housing`).

use citygen::City;
use citygen::overworld::Overworld;

/// A settlement of a world as the game builds it and its people as you find them: its
/// world seed, its name, the city and its people.
struct Seen {
    seed: u64,
    name: String,
    city: City,
    people: sim::World,
}

/// Every city and town of world `seed` but home, built from the world's seed and its
/// place in it as `outside::build_params` builds them, each seeded as you come in.
fn settlements_of(seed: u64) -> Vec<Seen> {
    let world = Overworld::generate(crate::outside::world_params(seed)).unwrap();
    let people = sim::People::load().unwrap();
    let base = crate::app::AppState::new().params;
    let home = world.cities.iter().position(|c| c.home).unwrap_or(0) as u8;
    let mut out = Vec::new();
    for (i, c) in world.cities.iter().enumerate() {
        let i = i as u8;
        let own = |t: Option<u8>| {
            let at = (u64::from(i) << 8) | t.map_or(0, |t| u64::from(t) + 1);
            base.clone()
                .with_seed(citygen::rng::hash64(world.seed ^ 0xC17E, at) % 1_000_000_007)
        };
        let mut all = Vec::new();
        if i != home {
            all.push((c.name.clone(), Some(citygen::sited(&world, i, own(None)))));
        }
        for t in 0..c.towns.len() as u8 {
            let name = format!("{}'s town {t}", c.name);
            all.push((name, citygen::sited_town(&world, i, t, own(Some(t)))));
        }
        for (name, params) in all {
            let Some(params) = params else { continue };
            let Ok(city) = citygen::Generator::run(params) else {
                continue;
            };
            let people = crate::outside::seed_people(&city, &people, 0, 4 * 60);
            out.push(Seen {
                seed,
                name,
                city,
                people,
            });
        }
    }
    out
}

/// The settlements of worlds `worlds`, built side by side.
fn settlements(worlds: std::ops::Range<u64>) -> Vec<Seen> {
    const THREADS: u64 = 8;
    std::thread::scope(|s| {
        let all: Vec<_> = (0..THREADS)
            .map(|k| {
                let worlds = worlds.clone();
                s.spawn(move || {
                    worlds
                        .filter(|w| w % THREADS == k)
                        .flat_map(settlements_of)
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        all.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// The beds of `c`'s housing, as the generator counts them, and its housing buildings.
fn beds(c: &City) -> (usize, usize) {
    c.buildings
        .iter()
        .map(|b| citygen::generate::housing::beds(c, b))
        .filter(|n| *n > 0)
        .fold((0, 0), |(beds, n), b| (beds + b, n + 1))
}

/// Whether `c` has no building off the Strip: nobody lives on the Strip, its rooms let
/// by the hour, so a city all strip has no homes (seed 71's Lowspire, 49's
/// Hollowspire, worlds the test doesn't take).
fn all_strip(c: &City) -> bool {
    c.buildings
        .iter()
        .all(|b| c.district(b.district).theme == citygen::Theme::NeonStrip)
}

/// Whether every building of `c` off the Strip but its clinic is housing already: a
/// city all strip but a street or two houses as many as it can (seed 7's Newvale).
fn all_flats(c: &City) -> bool {
    c.buildings.iter().all(|b| {
        c.district(b.district).theme == citygen::Theme::NeonStrip
            || b.kind == citygen::BuildingKind::Clinic
            || citygen::generate::housing::beds(c, b) > 0
    })
}

/// What's wrong with how `s` houses its people, if anything.
fn unhoused(s: &Seen) -> Option<String> {
    let people = s.people.people();
    let homes: std::collections::BTreeSet<citygen::BuildingId> =
        people.iter().filter_map(|p| p.home).collect();
    let homeless = people.iter().filter(|p| p.home.is_none()).count();
    let (beds, housing) = beds(&s.city);
    let n = people.len();
    let at = format!("seed {} {}: {n} people", s.seed, s.name);
    if all_strip(&s.city) {
        return None;
    }
    // nine in ten have a home, or all but two in a town of a few
    if homeless > (n / 10).max(2) {
        return Some(format!("{at}, {homeless} with no home"));
    }
    if n >= 50 && homes.len() < 2 {
        return Some(format!("{at} in {} building", homes.len()));
    }
    if n >= 50 && 2 * beds < n && !all_flats(&s.city) {
        return Some(format!("{at}, {beds} beds in {housing} buildings"));
    }
    None
}

#[test]
fn every_city_and_town_houses_its_people() {
    let all = settlements(0..30);
    assert!(all.len() > 300, "{} settlements", all.len());
    let wrong: Vec<String> = all.iter().filter_map(unhoused).collect();
    assert!(
        wrong.is_empty(),
        "{} of {}: {wrong:#?}",
        wrong.len(),
        all.len()
    );
}

/// How the cities and towns of 90 worlds house their people: a sweep (release:
/// `cargo test -p strata-zero --release -- --ignored of_90_worlds_houses`).
#[test]
#[ignore]
fn every_city_and_town_of_90_worlds_houses_its_people() {
    let all = settlements(0..90);
    let town = |s: &&Seen| s.name.contains("'s town");
    let of = |s: &Seen| s.people.people().len();
    let homeless = |s: &Seen| {
        s.people
            .people()
            .iter()
            .filter(|p| p.home.is_none())
            .count()
    };
    let total: usize = all.iter().map(of).sum();
    let out: usize = all.iter().map(homeless).sum();
    let none: Vec<&Seen> = all
        .iter()
        .filter(|s| of(s) > 0 && homeless(s) == of(s))
        .collect();
    let one = all
        .iter()
        .filter(|s| of(s) >= 50 && beds(&s.city).1 < 2)
        .count();
    let short = all
        .iter()
        .filter(|s| of(s) >= 50 && 2 * beds(&s.city).0 < of(s))
        .count();
    let cars: usize = all.iter().map(|s| s.people.cars().len()).sum();
    let carless = all
        .iter()
        .filter(|s| of(s) > 0 && s.people.cars().is_empty())
        .count();
    println!(
        "{} settlements ({} cities), {total} people, {} with a home",
        all.len(),
        all.iter().filter(|s| !town(s)).count(),
        total - out
    );
    println!(
        "nobody housed: {} settlements ({} cities), {} people",
        none.len(),
        none.iter().filter(|s| !town(s)).count(),
        none.iter().map(|s| of(s)).sum::<usize>()
    );
    println!("50 or more people, one housing building or none: {one}");
    println!("50 or more people, beds for under half: {short}");
    println!("cars: {cars}; settlements with people and no car: {carless}");
    let mut worst: Vec<&Seen> = all.iter().filter(|s| unhoused(s).is_some()).collect();
    worst.sort_by_key(|s| std::cmp::Reverse(of(s)));
    for s in worst.iter().take(12) {
        println!("  {}", unhoused(s).unwrap_or_default());
    }
}
