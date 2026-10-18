//! People: who they are, where they live and work, and the day each one keeps.

use crate::content::{DAY, Hours, Leisure, People, Profession, Tint};
use crate::places::Places;
use citygen::world::{Building, BuildingKind, City, Theme};
use citygen::{BuildingId, Hue, Pos};
use rand::Rng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PersonId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CarId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HouseholdId(pub u32);

/// How many people, and how many cars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimParams {
    pub density: f32,
    pub car_share: f32,
}

impl Default for SimParams {
    fn default() -> Self {
        SimParams {
            density: 1.0,
            car_share: 0.25,
        }
    }
}

/// Where someone works.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Work {
    Building(BuildingId),
    Spot(Pos),
    Roaming,
}

/// What a schedule segment heads for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Home,
    Work,
    Leisure(Leisure),
}

/// From minute `at` of the day, the person heads for `goal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub at: u16,
    pub goal: Goal,
}

/// The goal in force at a minute of the day: the latest segment at or before it,
/// wrapping to the last segment before the day's first.
pub fn goal_at(schedule: &[Segment], minute_of_day: u16) -> Goal {
    let m = minute_of_day % DAY;
    schedule
        .iter()
        .filter(|s| s.at <= m)
        .max_by_key(|s| s.at)
        .or_else(|| schedule.iter().max_by_key(|s| s.at))
        .map(|s| s.goal)
        .unwrap_or(Goal::Home)
}

/// What someone standing on the street is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Working,
    Loitering,
    Chatting,
    Queueing,
}

/// Where a trip ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest {
    Building(BuildingId),
    Spot(Pos),
}

/// One stretch of a trip: a path, on foot or in a car.
#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    pub path: Vec<Pos>,
    pub car: Option<CarId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trip {
    pub from: Pos,
    pub depart: u64,
    pub legs: Vec<Leg>,
    pub goal: Goal,
    pub dest: Dest,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Whereabouts {
    Inside(BuildingId),
    At { pos: Pos, activity: Activity },
    Travelling(Trip),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    pub profession: String,
    pub home: Option<BuildingId>,
    pub work: Work,
    pub car: Option<CarId>,
    pub household: HouseholdId,
    pub regular: Option<BuildingId>,
    /// Rolled at seeding: this person's leisure is the Strip.
    pub client: bool,
    /// Start minute of the shift or day this person keeps; colleagues share it.
    pub shift_start: u16,
    /// The span they work, as the profession's hours give it.
    pub shift: (u16, u16),
    pub schedule: Vec<Segment>,
    pub goal: Goal,
    pub whereabouts: Whereabouts,
    pub line: String,
    pub tint: Option<Tint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    pub id: CarId,
    pub owner: PersonId,
    /// Where the car stands, or stood when its owner set off for it.
    pub pos: Pos,
    pub hue: Hue,
    pub parked: bool,
    /// The kerb a car in flight is bound for: reserved from dispatch.
    pub bound: Option<Pos>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Population {
    pub people: Vec<Person>,
    pub cars: Vec<Car>,
    pub households: Vec<Vec<PersonId>>,
}

const CAR_HUES: [Hue; 7] = [
    Hue::Cyan,
    Hue::Magenta,
    Hue::Orange,
    Hue::Yellow,
    Hue::Blue,
    Hue::Red,
    Hue::White,
];

fn scaled(rng: &mut ChaCha8Rng, range: (u8, u8), density: f32) -> usize {
    let n = rng.random_range(range.0..=range.1) as f32 * density;
    let whole = n.floor();
    let extra = if rng.random::<f32>() < n - whole {
        1.0
    } else {
        0.0
    };
    (whole + extra) as usize
}

/// How many residents a building houses; a megablock scales with its footprint.
fn housing_capacity(b: &Building) -> usize {
    match b.kind {
        BuildingKind::Apartment => 8,
        BuildingKind::Megablock => (b.footprint.w as usize * b.footprint.h as usize / 6).max(30),
        BuildingKind::Hotel => 6,
        _ => 0,
    }
}

/// A job to fill: the profession, where, and which shift.
struct Opening<'a> {
    profession: &'a Profession,
    work: Work,
    shift: (u16, u16),
    /// Where the job physically is, for choosing a home nearby.
    at: Pos,
}

/// The whole population from the city's buildings, deterministic from `rng`.
pub fn seed_population(
    city: &City,
    people: &People,
    places: &Places,
    params: &SimParams,
    names: &[String],
    rng: &mut ChaCha8Rng,
) -> Population {
    let density = params.density.max(0.0);
    let mut openings: Vec<Opening> = Vec::new();

    // Jobs in buildings.
    for b in &city.buildings {
        let Some(doors) = places.doors.get(&b.id) else {
            continue;
        };
        let at = doors[0];
        for pr in people
            .professions
            .iter()
            .filter(|p| p.works_at.contains(&b.kind))
        {
            let shifts = pr.hours.spans();
            let per_shift = matches!(pr.hours, Hours::Shifts(_));
            let muscle = pr.id == "door_muscle";
            let post = places.posts.get(&b.id).copied().filter(|_| muscle);
            let work = match post {
                Some(p) => Work::Spot(p),
                None if pr.id == "courier" => Work::Roaming,
                None => Work::Building(b.id),
            };
            let at = post.unwrap_or(at);
            if per_shift {
                for &shift in &shifts {
                    // Muscle keep the tower and office doors by day, the club doors by night.
                    let night = shift.0 >= 720;
                    if muscle && night != (b.kind == BuildingKind::Club) {
                        continue;
                    }
                    for _ in 0..scaled(rng, pr.staff, density) {
                        openings.push(Opening {
                            profession: pr,
                            work,
                            shift,
                            at,
                        });
                    }
                }
            } else {
                for _ in 0..scaled(rng, pr.staff, density) {
                    openings.push(Opening {
                        profession: pr,
                        work,
                        shift: shifts[0],
                        at,
                    });
                }
            }
        }
    }
    // Jobs on the street.
    if let Some(sw) = people.profession("sex_worker") {
        // Sorted keys: hash maps iterate in no fixed order, and seeding must be stable.
        let mut districts: Vec<_> = places.beats.keys().copied().collect();
        districts.sort_by_key(|d| d.index());
        for d in districts {
            let beats = &places.beats[&d];
            for _ in 0..scaled(rng, sw.staff, density) {
                let spot = beats[rng.random_range(0..beats.len())];
                openings.push(Opening {
                    profession: sw,
                    work: Work::Spot(spot),
                    shift: sw.hours.spans()[0],
                    at: spot,
                });
            }
        }
    }
    if let Some(fx) = people.profession("fixer") {
        for d in &city.districts {
            let n = match d.theme {
                Theme::NeonStrip => 2,
                Theme::NightMarket => 1,
                _ => 0,
            };
            let tiles: Vec<Pos> = places
                .corners
                .get(&d.id)
                .cloned()
                .or_else(|| places.beats.get(&d.id).cloned())
                .unwrap_or_default();
            if tiles.is_empty() {
                continue;
            }
            for _ in 0..n {
                let spot = tiles[rng.random_range(0..tiles.len())];
                openings.push(Opening {
                    profession: fx,
                    work: Work::Spot(spot),
                    shift: fx.hours.spans()[0],
                    at: spot,
                });
            }
        }
    }
    // A city without a Strip or a Market still has one fixer, on the busiest corners.
    if let Some(fx) = people.profession("fixer")
        && !openings.iter().any(|o| o.profession.id == "fixer")
    {
        let best = city
            .districts
            .iter()
            .map(|d| (d.id, places.corners.get(&d.id).map_or(0, Vec::len)))
            .max_by_key(|(id, n)| (*n, std::cmp::Reverse(id.index())))
            .filter(|(_, n)| *n > 0)
            .map(|(id, _)| places.corners[&id].clone())
            .or_else(|| places.beats.values().find(|v| !v.is_empty()).cloned());
        if let Some(tiles) = best {
            let spot = tiles[rng.random_range(0..tiles.len())];
            openings.push(Opening {
                profession: fx,
                work: Work::Spot(spot),
                shift: fx.hours.spans()[0],
                at: spot,
            });
        }
    }
    if let Some(g) = people.profession("ganger") {
        let mut blocks: Vec<_> = places.turf.keys().copied().collect();
        blocks.sort_by_key(|b| b.index());
        for b in blocks {
            let ring = &places.turf[&b];
            for _ in 0..scaled(rng, g.staff, density) {
                let spot = ring[rng.random_range(0..ring.len())];
                openings.push(Opening {
                    profession: g,
                    work: Work::Spot(spot),
                    shift: g.hours.spans()[0],
                    at: spot,
                });
            }
        }
    }

    // Homes: housing outside the Strip, with capacity.
    let mut housing: Vec<(BuildingId, Pos, usize)> = city
        .buildings
        .iter()
        .filter(|b| housing_capacity(b) > 0)
        .filter(|b| city.district(b.district).theme != Theme::NeonStrip)
        .filter_map(|b| {
            let door = *places.doors.get(&b.id)?.first()?;
            let cap = ((housing_capacity(b) as f32 * density).round() as usize).max(1);
            Some((b.id, door, cap))
        })
        .collect();

    // Shared departure jitter per workplace and shift, so colleagues leave together.
    let mut group_jitter: HashMap<(Work, u16), i32> = HashMap::new();
    let mut pool: Vec<String> = names.to_vec();
    pool.shuffle(rng);
    let mut persons: Vec<Person> = Vec::with_capacity(openings.len());
    let mut by_home: HashMap<BuildingId, Vec<PersonId>> = HashMap::new();

    for (i, o) in openings.iter().enumerate() {
        let id = PersonId(i as u32);
        let name = pool.get(i).cloned().unwrap_or_else(|| {
            format!(
                "{}-{}",
                pool[i % pool.len().max(1)],
                i / pool.len().max(1) + 1
            )
        });
        // The nearest housing with room; corp drones take anything, others prefer close.
        let far_ok = o.profession.id == "corp_drone";
        let home = {
            let mut best: Option<(i32, usize)> = None;
            for (k, (_, door, cap)) in housing.iter().enumerate() {
                if *cap == 0 {
                    continue;
                }
                let d = door.manhattan(o.at);
                if !far_ok && d > 60 && best.is_some_and(|(bd, _)| bd <= 60) {
                    continue;
                }
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, k));
                }
            }
            best.map(|(_, k)| {
                housing[k].2 -= 1;
                housing[k].0
            })
        };
        if let Some(h) = home {
            by_home.entry(h).or_default().push(id);
        }
        let shared = *group_jitter
            .entry((o.work, o.shift.0))
            .or_insert_with(|| rng.random_range(0..=20));
        let (schedule, client) = schedule_for(o.profession, o.shift, shared, rng);
        let line = o.profession.lines[rng.random_range(0..o.profession.lines.len())].clone();
        let whereabouts = match home {
            Some(h) => Whereabouts::Inside(h),
            None => Whereabouts::At {
                pos: o.at,
                activity: Activity::Loitering,
            },
        };
        persons.push(Person {
            id,
            name,
            profession: o.profession.id.clone(),
            home,
            work: o.work,
            car: None,
            household: HouseholdId(0),
            regular: None,
            client,
            shift_start: o.shift.0,
            shift: o.shift,
            schedule,
            goal: Goal::Home,
            whereabouts,
            line,
            tint: o.profession.tint,
        });
    }

    // Households: the residents of a building, chunked into ones, twos and threes.
    let mut households: Vec<Vec<PersonId>> = Vec::new();
    let mut homes: Vec<(BuildingId, Vec<PersonId>)> = by_home.into_iter().collect();
    homes.sort_by_key(|(b, _)| b.index());
    for (_, members) in homes {
        let mut rest = members.as_slice();
        while !rest.is_empty() {
            let n = rng.random_range(1..=3).min(rest.len());
            let (h, tail) = rest.split_at(n);
            let hid = HouseholdId(households.len() as u32);
            for id in h {
                persons[id.0 as usize].household = hid;
            }
            households.push(h.to_vec());
            rest = tail;
        }
    }
    for p in persons.iter_mut().filter(|p| p.home.is_none()) {
        p.household = HouseholdId(households.len() as u32);
        households.push(vec![p.id]);
    }

    // Regulars: a bar in the home district, else the nearest bar.
    let bars: Vec<&citygen::world::Building> = city
        .buildings
        .iter()
        .filter(|b| b.kind == BuildingKind::Bar && places.doors.contains_key(&b.id))
        .collect();
    for p in persons.iter_mut() {
        if p.schedule
            .iter()
            .any(|s| s.goal == Goal::Leisure(Leisure::Bar))
        {
            let home_d = p.home.map(|h| city.buildings[h.index()].district);
            let pick = bars
                .iter()
                .find(|b| Some(b.district) == home_d)
                .or_else(|| bars.first());
            p.regular = pick.map(|b| b.id);
        }
    }

    // Cars: by share, wealth and profession, parked singly at the kerb nearest home.
    let mut cars: Vec<Car> = Vec::new();
    let mut taken: HashSet<Pos> = HashSet::new();
    let mut kerbs_by_dist: Vec<Pos> = places.kerbs.clone();
    for p in persons.iter_mut() {
        let Some(home) = p.home else { continue };
        let pr = people.profession(&p.profession).expect("known");
        let wealth = city.district(city.buildings[home.index()].district).wealth;
        let chance = (params.car_share + (wealth - 0.5) * 0.3 + pr.car_nudge * 0.3).clamp(0.0, 0.9);
        if rng.random::<f32>() >= chance {
            continue;
        }
        let door = places.doors[&home][0];
        kerbs_by_dist.sort_by_key(|k| k.manhattan(door));
        let lot = places
            .lots
            .iter()
            .flat_map(|(_, t)| t.iter().copied())
            .filter(|t| t.manhattan(door) <= 20 && !taken.contains(t))
            .min_by_key(|t| t.manhattan(door));
        let spot = lot.or_else(|| kerbs_by_dist.iter().copied().find(|k| !taken.contains(k)));
        let Some(spot) = spot else { continue };
        taken.insert(spot);
        let id = CarId(cars.len() as u32);
        cars.push(Car {
            id,
            owner: p.id,
            pos: spot,
            hue: CAR_HUES[rng.random_range(0..CAR_HUES.len())],
            parked: true,
            bound: None,
        });
        p.car = Some(id);
    }

    Population {
        people: persons,
        cars,
        households,
    }
}

/// A person's day from their profession's hours: out to work, then leisure or home,
/// with a shared departure jitter and a small personal one.
fn schedule_for(
    pr: &Profession,
    shift: (u16, u16),
    shared: i32,
    rng: &mut ChaCha8Rng,
) -> (Vec<Segment>, bool) {
    let jit = |rng: &mut ChaCha8Rng| -> i32 { rng.random_range(0..=20) };
    let wrap = |m: i32| ((m % DAY as i32) + DAY as i32) as u16 % DAY;
    let (start, end) = (shift.0 as i32, shift.1 as i32);
    let lead = match pr.hours {
        Hours::Day { .. } => 30,
        Hours::Shifts(_) => 20,
        Hours::Beat { .. } => 0,
    };
    let mut out = vec![Segment {
        at: wrap(start - lead + shared),
        goal: Goal::Work,
    }];
    let mut client = false;
    let mut leisure: Option<Leisure> = None;
    for (l, w) in &pr.leisure {
        if rng.random::<f32>() < *w {
            leisure = Some(*l);
            break;
        }
    }
    match leisure {
        Some(l) => {
            client = l == Leisure::Strip;
            let off = end + jit(rng);
            out.push(Segment {
                at: wrap(off),
                goal: Goal::Leisure(l),
            });
            out.push(Segment {
                at: wrap(off + rng.random_range(90..=150)),
                goal: Goal::Home,
            });
        }
        None => out.push(Segment {
            at: wrap(end + jit(rng)),
            goal: Goal::Home,
        }),
    }
    (out, client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::People;
    use crate::places::Places;
    use citygen::world::{BuildingKind, Theme};
    use citygen::{FlavorPack, GenParams, Generator, SizePreset};
    use rand::SeedableRng;
    use std::collections::{HashMap, HashSet};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    fn seed(city: &City, density: f32) -> Population {
        let people = People::load().unwrap();
        let places = Places::derive(city);
        let names = FlavorPack::load("default").unwrap().people;
        let params = SimParams {
            density,
            ..SimParams::default()
        };
        let mut rng = ChaCha8Rng::seed_from_u64(city.seed);
        seed_population(city, &people, &places, &params, &names, &mut rng)
    }

    #[test]
    fn a_city_seeds_a_population_that_fits_its_buildings() {
        let c = city();
        let people = People::load().unwrap();
        let places = Places::derive(&c);
        let pop = seed(&c, 1.0);
        let n = pop.people.len();
        assert!((80..=400).contains(&n), "{n} people");
        for p in &pop.people {
            let pr = people
                .profession(&p.profession)
                .expect("a known profession");
            match &p.work {
                Work::Building(b) => assert!(
                    pr.works_at.contains(&c.buildings[b.index()].kind),
                    "{} at a {:?}",
                    p.name,
                    c.buildings[b.index()].kind
                ),
                Work::Spot(spot) => assert!(
                    pr.works_at.is_empty()
                        || (p.profession == "door_muscle"
                            && places.posts.values().any(|q| q == spot)),
                    "{} has a spot job at {spot:?}",
                    p.profession
                ),
                Work::Roaming => assert_eq!(p.profession, "courier"),
            }
            if let Some(h) = p.home {
                let b = &c.buildings[h.index()];
                assert!(
                    matches!(
                        b.kind,
                        BuildingKind::Apartment | BuildingKind::Megablock | BuildingKind::Hotel
                    ),
                    "{:?}",
                    b.kind
                );
                assert_ne!(
                    c.district(b.district).theme,
                    Theme::NeonStrip,
                    "nobody lives on the Strip"
                );
            }
            assert!(!p.name.is_empty() && !p.line.is_empty());
            assert!(!p.schedule.is_empty());
        }
        let by_prof: HashMap<&str, usize> = pop.people.iter().fold(HashMap::new(), |mut m, p| {
            *m.entry(p.profession.as_str()).or_default() += 1;
            m
        });
        // Gangers hold megablock turf, so only a city with a megablock has them.
        let mut expected = vec!["shopkeeper", "sex_worker", "corp_drone", "line_worker"];
        if !places.turf.is_empty() {
            expected.push("ganger");
        }
        for id in expected {
            assert!(by_prof.get(id).is_some_and(|n| *n > 0), "{id}: {by_prof:?}");
        }
        let housed = pop.people.iter().filter(|p| p.home.is_some()).count();
        assert!(housed * 10 >= n * 9, "{housed} of {n} housed");
    }

    #[test]
    fn seeding_is_deterministic_and_scales_with_density() {
        let c = city();
        let a = seed(&c, 1.0);
        let b = seed(&c, 1.0);
        assert_eq!(a, b);
        let big = seed(&c, 2.0);
        assert!(
            big.people.len() as f32 >= a.people.len() as f32 * 1.6,
            "{} vs {}",
            big.people.len(),
            a.people.len()
        );
    }

    #[test]
    fn households_share_a_home_and_colleagues_a_departure() {
        let c = city();
        let pop = seed(&c, 1.0);
        for h in &pop.households {
            assert!((1..=3).contains(&h.len()));
            let homes: HashSet<_> = h.iter().map(|id| pop.people[id.0 as usize].home).collect();
            assert_eq!(homes.len(), 1, "one home per household");
        }
        let mut by_work: HashMap<(BuildingId, u16), Vec<u16>> = HashMap::new();
        for p in &pop.people {
            if let Work::Building(b) = p.work
                && let Some(seg) = p.schedule.iter().find(|s| s.goal == Goal::Work)
            {
                let shift = p.shift_start;
                by_work.entry((b, shift)).or_default().push(seg.at);
            }
        }
        let mut checked = 0;
        for (_, ats) in by_work.iter().filter(|(_, v)| v.len() > 1) {
            let lo = *ats.iter().min().unwrap();
            let hi = *ats.iter().max().unwrap();
            assert!(hi - lo <= 10, "colleagues depart together: {ats:?}");
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn the_night_shift_wraps_midnight() {
        let s = vec![
            Segment {
                at: 1300,
                goal: Goal::Work,
            },
            Segment {
                at: 370,
                goal: Goal::Home,
            },
        ];
        assert_eq!(goal_at(&s, 23 * 60), Goal::Work);
        assert_eq!(goal_at(&s, 3 * 60), Goal::Work);
        assert_eq!(goal_at(&s, 7 * 60), Goal::Home);
        assert_eq!(goal_at(&s, 21 * 60), Goal::Home);
        let night: Vec<&Person> = seed(&city(), 1.0)
            .people
            .iter()
            .filter(|p| p.profession == "line_worker" && p.shift_start == 1320)
            .cloned()
            .collect::<Vec<Person>>()
            .leak()
            .iter()
            .collect();
        assert!(!night.is_empty(), "someone works nights");
        for p in night {
            assert_eq!(
                goal_at(&p.schedule, 23 * 60),
                Goal::Work,
                "{:?}",
                p.schedule
            );
            assert_eq!(goal_at(&p.schedule, 3 * 60), Goal::Work);
            assert!(matches!(
                goal_at(&p.schedule, 8 * 60),
                Goal::Home | Goal::Leisure(_)
            ));
        }
    }

    #[test]
    fn a_city_without_housing_still_seeds_without_panicking() {
        let mut c = city();
        for b in &mut c.buildings {
            if matches!(
                b.kind,
                BuildingKind::Apartment | BuildingKind::Megablock | BuildingKind::Hotel
            ) {
                b.kind = BuildingKind::Warehouse;
            }
        }
        let pop = seed(&c, 1.0);
        assert!(!pop.people.is_empty());
        assert!(pop.people.iter().all(|p| p.home.is_none()));
        assert!(
            pop.people
                .iter()
                .all(|p| matches!(p.whereabouts, Whereabouts::At { .. }))
        );
    }

    #[test]
    fn cars_are_parked_singly_at_kerbs_near_their_owners() {
        let c = city();
        let places = Places::derive(&c);
        let pop = seed(&c, 1.0);
        let n = pop.people.len() as f32;
        let owners = pop.people.iter().filter(|p| p.car.is_some()).count() as f32;
        assert!((owners / n - 0.25).abs() < 0.15, "car share {}", owners / n);
        let mut tiles = HashSet::new();
        for car in &pop.cars {
            assert!(car.parked);
            assert!(
                places.is_kerb(car.pos) || places.lots.iter().any(|(_, t)| t.contains(&car.pos))
            );
            assert!(tiles.insert(car.pos), "two cars on {:?}", car.pos);
            assert_eq!(pop.people[car.owner.0 as usize].car, Some(car.id));
        }
    }

    #[test]
    fn the_beat_falls_back_when_the_strip_is_gone() {
        let mut c = city();
        for d in &mut c.districts {
            if d.theme == Theme::NeonStrip {
                d.theme = Theme::OldTown;
            }
        }
        let pop = seed(&c, 1.0);
        let places = Places::derive(&c);
        let workers: Vec<&Person> = pop
            .people
            .iter()
            .filter(|p| p.profession == "sex_worker")
            .collect();
        assert!(!workers.is_empty());
        for w in workers {
            let Work::Spot(pos) = w.work else {
                panic!("a beat")
            };
            assert!(places.beats.values().any(|v| v.contains(&pos)));
        }
    }

    #[test]
    fn housing_capacity_of_a_megablock_scales_with_its_footprint() {
        use citygen::geom::Rect;
        use citygen::ids::{BuildingId, DistrictId};
        use citygen::world::{HeightClass, StrataSpan};
        let mega = |w, h| Building {
            id: BuildingId(0),
            kind: BuildingKind::Megablock,
            district: DistrictId(0),
            footprint: Rect::new(0, 0, w, h),
            span: StrataSpan::from_class(HeightClass::Tall, false),
            name: None,
        };
        assert_eq!(housing_capacity(&mega(20, 18)), 60);
        assert_eq!(housing_capacity(&mega(6, 5)), 30);
    }

    #[test]
    fn every_city_has_a_fixer() {
        // Cities whose districts hold no Neon Strip or Night Market used to have none;
        // sweep a handful at four districts and demand a fixer in each.
        let mut without = 0;
        for s in [1u64, 2, 3, 4, 5, 6, 7, 11, 13, 17] {
            let c = Generator::run(GenParams {
                seed: s,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 4,
                ..Default::default()
            })
            .unwrap();
            if !c
                .districts
                .iter()
                .any(|d| matches!(d.theme, Theme::NeonStrip | Theme::NightMarket))
            {
                without += 1;
            }
            let p = seed(&c, 1.0);
            assert!(
                p.people.iter().any(|x| x.profession == "fixer"),
                "seed {s} has no fixer"
            );
        }
        assert!(
            without > 0,
            "no seed in the sweep lacks a Strip and a Market"
        );
    }

    #[test]
    fn a_person_keeps_their_shift() {
        let c = city();
        let p = seed(&c, 1.0);
        let fx = p.people.iter().find(|x| x.profession == "fixer").unwrap();
        assert_eq!(fx.shift, (1080, 240));
        assert_eq!(fx.shift_start, 1080);
    }
}
