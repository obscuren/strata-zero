//! The world and its minute: who leaves, who arrives, who stands where, and what the
//! player can see and ask.

use crate::content::{DAY, Leisure, People};
use crate::paths::{drive_path, walk_cost, walk_path};
use crate::people::{
    Activity, Car, CarId, Dest, Goal, Leg, Person, PersonId, Population, SimParams, Trip,
    Whereabouts, Work, goal_at, seed_population,
};
use crate::places::Places;
use citygen::world::{BuildingKind, City, Theme};
use citygen::{BuildingId, FlavorPack, Hue, Pos, Rect};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet, VecDeque};

/// Ticks in a game minute, as the HUD's clock keeps it.
pub const TICKS_PER_MINUTE: u64 = 90;
/// Ticks a walker spends per tile.
pub const WALK_TICKS: u64 = 3;
/// Ticks a car spends per tile.
pub const DRIVE_TICKS: u64 = 1;
/// A trip longer than this, on foot, is driven when there is a car.
pub const DRIVE_FROM: i32 = 60;

/// A trip someone wants; `path_some` turns it into legs.
#[derive(Clone, Debug, PartialEq)]
pub struct TripRequest {
    pub who: PersonId,
    pub dest: Dest,
    pub goal: Goal,
}

/// What the player can see of someone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoverKind {
    Walker {
        id: PersonId,
        tint: Option<crate::content::Tint>,
    },
    Car {
        id: CarId,
        hue: Hue,
        parked: bool,
    },
    Chat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mover {
    pub pos: Pos,
    pub kind: MoverKind,
}

/// What talking to someone, or inspecting them, shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub name: String,
    pub profession: String,
    pub doing: String,
    pub line: String,
    pub relation: Option<String>,
}

/// A client and a sex worker inside a hotel until a minute.
#[derive(Clone, Debug, PartialEq)]
struct Date {
    client: PersonId,
    worker: PersonId,
    hotel: BuildingId,
    until: u64,
    beat: Pos,
}

pub struct World {
    people: Vec<Person>,
    cars: Vec<Car>,
    households: Vec<Vec<PersonId>>,
    places: Places,
    labels: HashMap<String, String>,
    night_lines: HashMap<String, String>,
    pending: VecDeque<TripRequest>,
    /// The last tick advanced to.
    tick: u64,
    /// The last minute stepped, counted from tick 0.
    minute: u64,
    start_minute: u16,
    open: HashSet<BuildingId>,
    chats: Vec<(Pos, u64)>,
    dates: Vec<Date>,
    rng: ChaCha8Rng,
}

impl World {
    /// The city's people at `start_minute` of the day, placed where their schedules say.
    pub fn seed(city: &City, people: &People, params: &SimParams, start_minute: u16) -> World {
        let places = Places::derive(city);
        let names = FlavorPack::load(&city.params.flavor)
            .map(|f| f.people)
            .unwrap_or_default();
        let mut rng = ChaCha8Rng::seed_from_u64(city.seed ^ 0x5EED_C17F);
        let Population {
            people: persons,
            cars,
            households,
        } = seed_population(city, people, &places, params, &names, &mut rng);
        let mut w = World {
            people: persons,
            cars,
            households,
            places,
            labels: people
                .professions
                .iter()
                .map(|p| (p.id.clone(), p.label.clone()))
                .collect(),
            night_lines: people
                .professions
                .iter()
                .filter_map(|p| p.night_line.clone().map(|l| (p.id.clone(), l)))
                .collect(),
            pending: VecDeque::new(),
            tick: 0,
            minute: 0,
            start_minute,
            open: HashSet::new(),
            chats: Vec::new(),
            dates: Vec::new(),
            rng,
        };
        w.place_everyone(city);
        w.refresh_open();
        w
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn minute_of_day(&self) -> u16 {
        ((self.start_minute as u64 + self.minute) % DAY as u64) as u16
    }

    pub fn people(&self) -> &[Person] {
        &self.people
    }

    pub fn cars(&self) -> &[Car] {
        &self.cars
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn is_open(&self, building: BuildingId) -> bool {
        self.open.contains(&building)
    }

    /// Everyone goes straight to where the clock says, no trips: the seed state.
    fn place_everyone(&mut self, city: &City) {
        let mod_ = self.minute_of_day();
        for i in 0..self.people.len() {
            let goal = goal_at(&self.people[i].schedule, mod_);
            let dest = self.dest_for(city, i, goal);
            self.people[i].goal = goal;
            self.people[i].whereabouts = match dest {
                Some(Dest::Building(b)) => Whereabouts::Inside(b),
                Some(Dest::Spot(pos)) => Whereabouts::At {
                    pos,
                    activity: if goal == Goal::Work {
                        Activity::Working
                    } else {
                        Activity::Loitering
                    },
                },
                None => self.people[i].whereabouts.clone(),
            };
        }
    }

    /// Where a goal takes a person: their home, their work, their bar, the Strip.
    fn dest_for(&mut self, city: &City, i: usize, goal: Goal) -> Option<Dest> {
        let p = &self.people[i];
        match goal {
            Goal::Home => match p.home {
                Some(h) => Some(Dest::Building(h)),
                None => match p.work {
                    Work::Spot(s) => Some(Dest::Spot(s)),
                    _ => None,
                },
            },
            Goal::Work => match p.work {
                Work::Building(b) => Some(Dest::Building(b)),
                Work::Spot(s) => Some(Dest::Spot(s)),
                Work::Roaming => self.random_workplace(city),
            },
            Goal::Leisure(Leisure::Bar) => p
                .regular
                .or_else(|| self.nearest_kind(city, i, BuildingKind::Bar))
                .map(Dest::Building),
            Goal::Leisure(Leisure::NoodleBar) => self
                .nearest_kind(city, i, BuildingKind::NoodleBar)
                .map(Dest::Building),
            Goal::Leisure(Leisure::Strip) => {
                let mut districts: Vec<_> = self.places.beats.keys().copied().collect();
                districts.sort_by_key(|d| d.index());
                let d = *districts.get(self.rng.random_range(0..districts.len().max(1)))?;
                let beats = &self.places.beats[&d];
                Some(Dest::Spot(beats[self.rng.random_range(0..beats.len())]))
            }
        }
    }

    fn random_workplace(&mut self, city: &City) -> Option<Dest> {
        let mut ids: Vec<BuildingId> = city
            .buildings
            .iter()
            .filter(|b| self.places.doors.contains_key(&b.id))
            .filter(|b| {
                !matches!(
                    b.kind,
                    BuildingKind::Apartment | BuildingKind::Megablock | BuildingKind::Hotel
                )
            })
            .map(|b| b.id)
            .collect();
        ids.sort_by_key(|b| b.index());
        if ids.is_empty() {
            return None;
        }
        Some(Dest::Building(ids[self.rng.random_range(0..ids.len())]))
    }

    fn anchor(&self, i: usize) -> Option<Pos> {
        match &self.people[i].whereabouts {
            Whereabouts::Inside(b) => self.places.doors.get(b).and_then(|d| d.first().copied()),
            Whereabouts::At { pos, .. } => Some(*pos),
            Whereabouts::Travelling(t) => t.legs.last().and_then(|l| l.path.last().copied()),
        }
    }

    fn nearest_kind(&self, city: &City, i: usize, kind: BuildingKind) -> Option<BuildingId> {
        let from = self.anchor(i)?;
        city.buildings
            .iter()
            .filter(|b| b.kind == kind)
            .filter_map(|b| self.places.doors.get(&b.id).map(|d| (b.id, d[0])))
            .min_by_key(|(_, d)| d.manhattan(from))
            .map(|(id, _)| id)
    }

    /// Asks for a trip; `path_some` makes it real.
    pub fn request(&mut self, who: PersonId, dest: Dest, goal: Goal) {
        self.pending.push_back(TripRequest { who, dest, goal });
    }

    /// The nearest kerb to `to` that no parked car holds.
    pub fn free_kerb_near(&self, city: &City, to: Pos, occupied: &HashSet<Pos>) -> Option<Pos> {
        let mut kerbs: Vec<Pos> = self
            .places
            .kerbs
            .iter()
            .copied()
            .filter(|k| !occupied.contains(k))
            .collect();
        kerbs.sort_by_key(|k| k.manhattan(to));
        kerbs
            .into_iter()
            .take(12)
            .find(|k| walk_path(city, to, *k).is_some())
            .or_else(|| {
                self.places
                    .nearest_kerb(city, to)
                    .filter(|k| !occupied.contains(k))
            })
    }

    fn parked_tiles(&self) -> HashSet<Pos> {
        self.cars
            .iter()
            .filter(|c| c.parked)
            .map(|c| c.pos)
            .collect()
    }

    /// Turns up to `budget` requests into trips, dropping any with no path.
    pub fn path_some(&mut self, city: &City, budget: usize) {
        for _ in 0..budget {
            let Some(req) = self.pending.pop_front() else {
                break;
            };
            let i = req.who.0 as usize;
            let Some(from) = self.anchor(i) else { continue };
            let to = match req.dest {
                Dest::Building(b) => {
                    let Some(doors) = self.places.doors.get(&b) else {
                        continue;
                    };
                    *doors
                        .iter()
                        .min_by_key(|d| d.manhattan(from))
                        .unwrap_or(&doors[0])
                }
                Dest::Spot(s) => s,
            };
            if from == to {
                self.arrive(i, req.dest, req.goal);
                continue;
            }
            let mut legs: Vec<Leg> = Vec::new();
            let car = self.people[i]
                .car
                .filter(|c| self.cars[c.0 as usize].parked);
            if let Some(car_id) = car
                && from.manhattan(to) > DRIVE_FROM
            {
                let car_pos = self.cars[car_id.0 as usize].pos;
                let mut occupied = self.parked_tiles();
                occupied.remove(&car_pos);
                if let Some(kerb) = self.free_kerb_near(city, to, &occupied)
                    && let Some(a) = walk_path(city, from, car_pos)
                    && let Some(b) = drive_path(city, car_pos, kerb)
                    && let Some(c) = walk_path(city, kerb, to)
                {
                    legs = vec![
                        Leg { path: a, car: None },
                        Leg {
                            path: b,
                            car: Some(car_id),
                        },
                        Leg { path: c, car: None },
                    ];
                    let car = &mut self.cars[car_id.0 as usize];
                    car.parked = false;
                    car.pos = kerb;
                }
            }
            if legs.is_empty() {
                match walk_path(city, from, to) {
                    Some(p) => legs.push(Leg { path: p, car: None }),
                    None => continue,
                }
            }
            self.people[i].whereabouts = Whereabouts::Travelling(Trip {
                from,
                depart: self.tick,
                legs,
                goal: req.goal,
                dest: req.dest,
            });
        }
    }

    fn arrive(&mut self, i: usize, dest: Dest, goal: Goal) {
        let p = &mut self.people[i];
        p.whereabouts = match dest {
            Dest::Building(b) => Whereabouts::Inside(b),
            Dest::Spot(pos) => Whereabouts::At {
                pos,
                activity: if goal == Goal::Work {
                    Activity::Working
                } else {
                    Activity::Loitering
                },
            },
        };
    }

    /// Ticks a trip takes end to end.
    fn trip_ticks(trip: &Trip) -> u64 {
        trip.legs
            .iter()
            .map(|l| {
                let tiles = l.path.len().saturating_sub(1) as u64;
                tiles
                    * if l.car.is_some() {
                        DRIVE_TICKS
                    } else {
                        WALK_TICKS
                    }
            })
            .sum()
    }

    /// Where a trip is after `elapsed` ticks, and the car being driven if any.
    fn trip_pos(trip: &Trip, elapsed: u64) -> (Pos, Option<CarId>) {
        let mut left = elapsed;
        for l in &trip.legs {
            let per = if l.car.is_some() {
                DRIVE_TICKS
            } else {
                WALK_TICKS
            };
            let tiles = l.path.len().saturating_sub(1) as u64;
            let span = tiles * per;
            if left < span || span == 0 {
                let idx = (left / per.max(1)).min(tiles) as usize;
                return (l.path[idx], l.car);
            }
            left -= span;
        }
        let last = trip
            .legs
            .last()
            .and_then(|l| l.path.last().copied())
            .unwrap_or(trip.from);
        (last, None)
    }

    /// Where a person is at `tick`, if on the street.
    fn position(&self, i: usize, tick: u64) -> Option<(Pos, Option<CarId>)> {
        match &self.people[i].whereabouts {
            Whereabouts::Inside(_) => None,
            Whereabouts::At { pos, .. } => Some((*pos, None)),
            Whereabouts::Travelling(t) => Some(Self::trip_pos(t, tick.saturating_sub(t.depart))),
        }
    }

    /// Runs the minute steps up to `tick`.
    pub fn advance(&mut self, city: &City, tick: u64) {
        self.tick = tick;
        let minute = tick / TICKS_PER_MINUTE;
        while self.minute < minute {
            self.minute += 1;
            self.step(city);
        }
    }

    fn refresh_open(&mut self) {
        self.open = self
            .people
            .iter()
            .filter(|p| p.goal == Goal::Work)
            .filter_map(|p| match (p.work, &p.whereabouts) {
                (Work::Building(b), Whereabouts::Inside(w)) if *w == b => Some(b),
                _ => None,
            })
            .collect();
    }

    fn step(&mut self, city: &City) {
        let now = self.tick.max(self.minute * TICKS_PER_MINUTE);
        let mod_ = self.minute_of_day();
        // Arrivals.
        for i in 0..self.people.len() {
            if let Whereabouts::Travelling(t) = &self.people[i].whereabouts
                && now.saturating_sub(t.depart) >= Self::trip_ticks(t)
            {
                let (dest, goal) = (t.dest, t.goal);
                for l in &t.legs {
                    if let Some(c) = l.car {
                        let car = &mut self.cars[c.0 as usize];
                        car.parked = true;
                        car.pos = *l.path.last().unwrap_or(&car.pos);
                    }
                }
                // A noodle bar at its rush: queue a minute at the door first.
                if let Dest::Building(b) = dest
                    && city.buildings[b.index()].kind == BuildingKind::NoodleBar
                    && goal != Goal::Work
                    && self.open.contains(&b)
                    && matches!(mod_, 1080..=1260 | 0..=120)
                    && !matches!(
                        self.people[i].whereabouts,
                        Whereabouts::At {
                            activity: Activity::Queueing,
                            ..
                        }
                    )
                {
                    let door = self.places.doors[&b][0];
                    self.people[i].whereabouts = Whereabouts::At {
                        pos: door,
                        activity: Activity::Queueing,
                    };
                    continue;
                }
                self.arrive(i, dest, goal);
            } else if let Whereabouts::At {
                activity: Activity::Queueing,
                ..
            } = self.people[i].whereabouts
                && let Some(b) = match self.people[i].goal {
                    Goal::Leisure(Leisure::NoodleBar) => {
                        self.nearest_kind(city, i, BuildingKind::NoodleBar)
                    }
                    _ => None,
                }
            {
                self.people[i].whereabouts = Whereabouts::Inside(b);
            }
        }
        // Dates ending: both come out.
        let mut ended = Vec::new();
        self.dates.retain(|d| {
            if d.until <= self.minute {
                ended.push(d.clone());
                false
            } else {
                true
            }
        });
        for d in ended {
            self.people[d.worker.0 as usize].whereabouts = Whereabouts::At {
                pos: d.beat,
                activity: Activity::Working,
            };
            let door = self.places.doors[&d.hotel][0];
            self.people[d.client.0 as usize].whereabouts = Whereabouts::At {
                pos: door,
                activity: Activity::Loitering,
            };
            self.people[d.client.0 as usize].goal = Goal::Work; // re-evaluated below, sends them home
        }
        // Departures: the schedule changed its mind, and the person is free to go.
        for i in 0..self.people.len() {
            if matches!(self.people[i].whereabouts, Whereabouts::Travelling(_)) {
                continue;
            }
            if self
                .dates
                .iter()
                .any(|d| d.client.0 as usize == i || d.worker.0 as usize == i)
            {
                continue;
            }
            let goal = goal_at(&self.people[i].schedule, mod_);
            let roaming = self.people[i].work == Work::Roaming
                && goal == Goal::Work
                && self.rng.random::<f32>() < 0.2;
            if goal != self.people[i].goal || roaming {
                self.people[i].goal = goal;
                if let Some(dest) = self.dest_for(city, i, goal) {
                    self.pending.push_back(TripRequest {
                        who: PersonId(i as u32),
                        dest,
                        goal,
                    });
                }
            }
        }
        // Spots breathe: a one-tile shuffle around the anchor now and then. Chats start.
        self.chats.retain(|(_, until)| *until > self.minute);
        let mut by_tile: HashMap<Pos, Vec<usize>> = HashMap::new();
        for i in 0..self.people.len() {
            let anchor = match self.people[i].work {
                Work::Spot(a) => Some(a),
                _ => None,
            };
            let roll = self.rng.random::<f32>();
            let pick = self.rng.random_range(0..5usize);
            if let Whereabouts::At { pos, activity } = &mut self.people[i].whereabouts {
                if *activity == Activity::Chatting {
                    *activity = Activity::Working;
                }
                if let Some(a) = anchor
                    && *activity == Activity::Working
                    && roll < 0.3
                {
                    let mut spots: Vec<Pos> = a
                        .neighbors4()
                        .chain(std::iter::once(a))
                        .filter(|n| walk_cost(city, *n, *n).is_some_and(|c| c == 1))
                        .collect();
                    spots.sort_by_key(|p| (p.y, p.x));
                    if !spots.is_empty() {
                        *pos = spots[pick % spots.len()];
                    }
                }
                by_tile.entry(*pos).or_default().push(i);
            }
        }
        let mut tiles: Vec<Pos> = by_tile.keys().copied().collect();
        tiles.sort_by_key(|p| (p.y, p.x));
        for t in tiles {
            let who = &by_tile[&t];
            if who.len() >= 2 && self.rng.random::<f32>() < 0.2 {
                for &i in who.iter().take(2) {
                    if let Whereabouts::At { activity, .. } = &mut self.people[i].whereabouts {
                        *activity = Activity::Chatting;
                    }
                }
                self.chats.push((t, self.minute + 1));
            }
        }
        // The Strip: a client beside a worker, and a hotel nearby.
        self.strip_dates(city);
        self.refresh_open();
    }

    fn strip_dates(&mut self, city: &City) {
        let workers: Vec<(usize, Pos)> = self
            .people
            .iter()
            .enumerate()
            .filter(|(_, p)| p.profession == "sex_worker")
            .filter_map(|(i, p)| match p.whereabouts {
                Whereabouts::At {
                    pos,
                    activity: Activity::Working,
                } => Some((i, pos)),
                _ => None,
            })
            .collect();
        let clients: Vec<(usize, Pos)> = self
            .people
            .iter()
            .enumerate()
            .filter(|(_, p)| p.client && p.goal == Goal::Leisure(Leisure::Strip))
            .filter_map(|(i, p)| match p.whereabouts {
                Whereabouts::At {
                    pos,
                    activity: Activity::Loitering,
                } => Some((i, pos)),
                _ => None,
            })
            .collect();
        let mut used: HashSet<usize> = HashSet::new();
        for (ci, cpos) in clients {
            let Some(&(wi, wpos)) = workers
                .iter()
                .filter(|(wi, _)| !used.contains(wi))
                .find(|(_, wpos)| wpos.manhattan(cpos) <= 3)
            else {
                continue;
            };
            if self.rng.random::<f32>() >= 0.3 {
                continue;
            }
            let Some(hotel) = city
                .buildings
                .iter()
                .filter(|b| {
                    b.kind == BuildingKind::Hotel
                        && city.district(b.district).theme == Theme::NeonStrip
                })
                .filter_map(|b| self.places.doors.get(&b.id).map(|d| (b.id, d[0])))
                .min_by_key(|(_, d)| d.manhattan(wpos))
                .map(|(id, _)| id)
            else {
                continue;
            };
            used.insert(wi);
            let until = self.minute + self.rng.random_range(20..=40);
            self.people[ci].whereabouts = Whereabouts::Inside(hotel);
            self.people[wi].whereabouts = Whereabouts::Inside(hotel);
            self.dates.push(Date {
                client: PersonId(ci as u32),
                worker: PersonId(wi as u32),
                hotel,
                until,
                beat: wpos,
            });
        }
    }

    /// Everyone and every car inside `bubble` at `tick`, with chat markers.
    pub fn visible(&self, bubble: Rect, tick: u64) -> Vec<Mover> {
        let mut out = Vec::new();
        let mut driving: HashMap<CarId, Pos> = HashMap::new();
        for (i, p) in self.people.iter().enumerate() {
            let Some((pos, car)) = self.position(i, tick) else {
                continue;
            };
            match car {
                Some(c) => {
                    driving.insert(c, pos);
                }
                None => {
                    if bubble.contains(pos) {
                        out.push(Mover {
                            pos,
                            kind: MoverKind::Walker {
                                id: p.id,
                                tint: p.tint,
                            },
                        });
                    }
                }
            }
        }
        for car in &self.cars {
            let (pos, parked) = match driving.get(&car.id) {
                Some(p) => (*p, false),
                None => (car.pos, true),
            };
            if bubble.contains(pos) {
                out.push(Mover {
                    pos,
                    kind: MoverKind::Car {
                        id: car.id,
                        hue: car.hue,
                        parked,
                    },
                });
            }
        }
        for (pos, _) in &self.chats {
            if bubble.contains(*pos) {
                out.push(Mover {
                    pos: *pos,
                    kind: MoverKind::Chat,
                });
            }
        }
        out
    }

    /// The walker standing on `pos` at `tick`, if any.
    pub fn person_at(&self, pos: Pos, tick: u64) -> Option<PersonId> {
        (0..self.people.len()).find_map(|i| match self.position(i, tick) {
            Some((p, None)) if p == pos => Some(self.people[i].id),
            _ => None,
        })
    }

    /// The car on `pos` at `tick`, parked or driving.
    pub fn car_at(&self, pos: Pos, tick: u64) -> Option<CarId> {
        for (i, p) in self.people.iter().enumerate() {
            if let Some((cp, Some(c))) = self.position(i, tick)
                && cp == pos
            {
                let _ = p;
                return Some(c);
            }
        }
        self.cars
            .iter()
            .find(|c| c.parked && c.pos == pos)
            .map(|c| c.id)
    }

    fn label(&self, profession: &str) -> String {
        self.labels
            .get(profession)
            .cloned()
            .unwrap_or_else(|| profession.replace('_', " "))
    }

    fn place_name(city: &City, b: BuildingId) -> String {
        let b = &city.buildings[b.index()];
        b.name
            .clone()
            .unwrap_or_else(|| format!("the {}", b.kind.label()))
    }

    /// What someone would tell you, and what they are up to.
    pub fn card(&self, id: PersonId, city: &City) -> Card {
        let p = &self.people[id.0 as usize];
        let at_work_spot = |pos: Pos| -> bool { p.work == Work::Spot(pos) };
        let doing = match (&p.whereabouts, p.goal) {
            (Whereabouts::Inside(b), Goal::Work) => match city.buildings[b.index()].kind {
                BuildingKind::Shop | BuildingKind::Market | BuildingKind::Pawn => {
                    "minding the shop".to_string()
                }
                BuildingKind::NoodleBar => "working the stall".to_string(),
                BuildingKind::Bar | BuildingKind::Club => "behind the bar".to_string(),
                BuildingKind::Clinic => "in the clinic".to_string(),
                BuildingKind::Temple => "at the temple".to_string(),
                _ => format!("working at {}", Self::place_name(city, *b)),
            },
            (Whereabouts::Inside(b), Goal::Home) if Some(*b) == p.home => "at home".to_string(),
            (Whereabouts::Inside(b), _) => format!("inside {}", Self::place_name(city, *b)),
            (Whereabouts::At { pos, activity }, _)
                if at_work_spot(*pos) || *activity == Activity::Working =>
            {
                match p.profession.as_str() {
                    "sex_worker" => "working the Strip".to_string(),
                    "fixer" => "holding the corner".to_string(),
                    "door_muscle" => "on the door".to_string(),
                    "ganger" => "on the block".to_string(),
                    _ => "out on the street".to_string(),
                }
            }
            (
                Whereabouts::At {
                    activity: Activity::Queueing,
                    ..
                },
                _,
            ) => "in line for noodles".to_string(),
            (
                Whereabouts::At {
                    activity: Activity::Chatting,
                    ..
                },
                _,
            ) => "talking".to_string(),
            (Whereabouts::At { .. }, Goal::Leisure(Leisure::Strip)) => {
                "looking for company".to_string()
            }
            (Whereabouts::At { .. }, _) => "hanging around".to_string(),
            (Whereabouts::Travelling(t), _) => match t.goal {
                Goal::Home => "heading home".to_string(),
                Goal::Work => match p.work {
                    Work::Roaming => "on a delivery".to_string(),
                    _ => "heading to work".to_string(),
                },
                Goal::Leisure(Leisure::Bar) => "off to the bar".to_string(),
                Goal::Leisure(Leisure::NoodleBar) => "off for noodles".to_string(),
                Goal::Leisure(Leisure::Strip) => "heading for the Strip".to_string(),
            },
        };
        let night = (18 * 60..DAY).contains(&self.minute_of_day()) || self.minute_of_day() < 5 * 60;
        let line = match (&p.whereabouts, p.profession.as_str(), night) {
            (Whereabouts::At { .. }, "door_muscle", true) => self
                .night_lines
                .get("door_muscle")
                .cloned()
                .unwrap_or_else(|| p.line.clone()),
            _ => p.line.clone(),
        };
        let relation = self
            .households
            .get(p.household.0 as usize)
            .and_then(|h| {
                h.iter()
                    .filter(|m| **m != p.id)
                    .map(|m| &self.people[m.0 as usize])
                    .find(|m| m.shift_start != p.shift_start)
                    .map(|m| format!("my housemate {} keeps the other shift", m.name))
            })
            .or_else(|| {
                p.regular
                    .map(|b| format!("you'll find me at {} after", Self::place_name(city, b)))
            });
        Card {
            name: p.name.clone(),
            profession: self.label(&p.profession),
            doing,
            line,
            relation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::People;
    use crate::paths::drive_cost;
    use citygen::world::{BuildingKind, Theme};
    use citygen::{GenParams, Generator, Rect, SizePreset, StratumKind, Terrain};
    use std::collections::HashSet;

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    fn world(c: &City) -> World {
        World::seed(c, &People::load().unwrap(), &SimParams::default(), 22 * 60)
    }

    /// Ticks from the 22:00 start to `hhmm` on day `day` (day 0 is the first night).
    fn tick_at(hhmm: u16, day: u64) -> u64 {
        let start = 22 * 60u64;
        let target = day * 1440 + hhmm as u64;
        let from = if target >= start {
            target - start
        } else {
            target + 1440 - start
        };
        from * TICKS_PER_MINUTE
    }

    /// Advances minute by minute, pathing everything each minute.
    fn run_to(w: &mut World, c: &City, tick: u64) {
        let mut t = w.tick();
        while t < tick {
            t += TICKS_PER_MINUTE;
            w.advance(c, t.min(tick));
            w.path_some(c, usize::MAX);
        }
    }

    fn one(w: &World, f: impl Fn(&Person) -> bool) -> &Person {
        w.people().iter().find(|p| f(p)).expect("someone fits")
    }

    #[test]
    fn a_shopkeeper_minds_the_shop_at_noon_and_sleeps_at_three() {
        let c = city();
        let mut w = world(&c);
        let id = one(&w, |p| {
            p.profession == "shopkeeper"
                && matches!(p.work, Work::Building(b) if c.buildings[b.index()].kind != BuildingKind::Pawn)
                && p.home.is_some()
        })
        .id;
        run_to(&mut w, &c, tick_at(3 * 60, 1));
        let p = &w.people()[id.0 as usize];
        assert_eq!(
            p.whereabouts,
            Whereabouts::Inside(p.home.unwrap()),
            "{:?}",
            p.whereabouts
        );
        run_to(&mut w, &c, tick_at(12 * 60, 1));
        let p = &w.people()[id.0 as usize];
        let Work::Building(shop) = p.work else {
            unreachable!()
        };
        assert_eq!(
            p.whereabouts,
            Whereabouts::Inside(shop),
            "{:?}",
            p.whereabouts
        );
        assert!(w.is_open(shop));
        let card = w.card(id, &c);
        assert_eq!(card.profession, "shopkeeper");
        assert!(card.doing.contains("shop"), "{}", card.doing);
        assert!(!card.line.is_empty());
        run_to(&mut w, &c, tick_at(3 * 60, 2));
        assert!(!w.is_open(shop));
    }

    #[test]
    fn the_strip_is_busy_at_eleven_and_dead_at_noon() {
        let c = city();
        let mut w = world(&c);
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        let working = |w: &World| {
            w.people()
                .iter()
                .filter(|p| p.profession == "sex_worker")
                .filter(|p| {
                    matches!(
                        p.whereabouts,
                        Whereabouts::At {
                            activity: Activity::Working,
                            ..
                        }
                    )
                })
                .count()
        };
        assert!(working(&w) > 0, "the beat is worked at eleven");
        run_to(&mut w, &c, tick_at(12 * 60, 1));
        assert_eq!(working(&w), 0, "nobody works the beat at noon");
        assert!(
            w.people()
                .iter()
                .filter(|p| p.profession == "sex_worker")
                .all(|p| p.goal == Goal::Home),
            "every worker is off at noon"
        );
    }

    #[test]
    fn a_driver_takes_the_car_and_parks_it_near_work() {
        let c = city();
        let mut w = world(&c);
        let places = Places::derive(&c);
        let driver = one(&w, |p| {
            p.car.is_some()
                && matches!((p.home, p.work), (Some(h), Work::Building(b))
                    if places.doors[&h][0].manhattan(places.doors[&b][0]) > 60)
                && p.profession != "line_worker"
        })
        .clone();
        let Work::Building(work) = driver.work else {
            unreachable!()
        };
        let mut drove = false;
        let mut t = w.tick();
        let end = tick_at(12 * 60, 1);
        while t < end {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            if let Whereabouts::Travelling(trip) = &w.people()[driver.id.0 as usize].whereabouts
                && trip.legs.iter().any(|l| l.car.is_some())
            {
                drove = true;
                for l in &trip.legs {
                    for &p in &l.path {
                        if l.car.is_some() {
                            assert!(drive_cost(&c, p, p).is_some(), "{p:?} drivable");
                        } else {
                            assert!(walk_cost(&c, p, p).is_some(), "{p:?} walkable");
                        }
                    }
                }
            }
            let mut seen = HashSet::new();
            for car in w.cars().iter().filter(|car| car.parked) {
                assert!(seen.insert(car.pos), "two parked cars on {:?}", car.pos);
            }
        }
        assert!(drove, "a long commute with a car is driven");
        let p = &w.people()[driver.id.0 as usize];
        assert_eq!(p.whereabouts, Whereabouts::Inside(work));
        let car = &w.cars()[driver.car.unwrap().0 as usize];
        assert!(car.parked);
        assert!(places.is_kerb(car.pos) || places.lots.iter().any(|(_, t)| t.contains(&car.pos)));
        assert!(
            car.pos.manhattan(places.doors[&work][0]) <= 40,
            "parked near work: {:?}",
            car.pos
        );
    }

    #[test]
    fn colleagues_arrive_together_and_the_night_shift_keeps_its_days() {
        let c = city();
        let mut w = world(&c);
        let night: Vec<PersonId> = w
            .people()
            .iter()
            .filter(|p| p.profession == "line_worker" && p.shift_start == 1320 && p.home.is_some())
            .map(|p| p.id)
            .collect();
        assert!(night.len() >= 2, "{} night workers", night.len());
        for day in 0..2u64 {
            run_to(&mut w, &c, tick_at(23 * 60 + 30, day));
            for id in &night {
                let p = &w.people()[id.0 as usize];
                let Work::Building(b) = p.work else {
                    unreachable!()
                };
                assert_eq!(
                    p.whereabouts,
                    Whereabouts::Inside(b),
                    "day {day} {}: {:?}",
                    p.name,
                    p.whereabouts
                );
            }
            run_to(&mut w, &c, tick_at(9 * 60, day + 1));
            for id in &night {
                let p = &w.people()[id.0 as usize];
                assert_eq!(
                    p.whereabouts,
                    Whereabouts::Inside(p.home.unwrap()),
                    "day {day} {} is home by nine",
                    p.name
                );
            }
        }
    }

    #[test]
    fn an_unreachable_destination_drops_the_trip() {
        let c = city();
        let mut w = world(&c);
        let water = (0..c.size.h as i32)
            .flat_map(|y| (0..c.size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| {
                c.tile(StratumKind::Street, *p)
                    .is_some_and(|t| t.terrain == Terrain::Water)
            })
            .expect("a bay");
        let id = one(&w, |p| p.home.is_some()).id;
        w.request(id, Dest::Spot(water), Goal::Work);
        w.path_some(&c, usize::MAX);
        let p = &w.people()[id.0 as usize];
        assert!(
            !matches!(p.whereabouts, Whereabouts::Travelling(_)),
            "{:?}",
            p.whereabouts
        );
        assert_eq!(w.pending(), 0);
    }

    #[test]
    fn two_cars_bound_for_one_kerb_park_apart() {
        let c = city();
        let w = world(&c);
        let places = Places::derive(&c);
        let door = *places.doors.values().next().unwrap().first().unwrap();
        let mut occupied: HashSet<Pos> = w.cars().iter().map(|car| car.pos).collect();
        let a = w.free_kerb_near(&c, door, &occupied).expect("a kerb");
        occupied.insert(a);
        let b = w.free_kerb_near(&c, door, &occupied).expect("another kerb");
        assert_ne!(a, b);
        assert!(places.is_kerb(a) && places.is_kerb(b));
    }

    #[test]
    fn visible_movers_sit_on_their_paths_inside_the_rect() {
        let c = city();
        let mut w = world(&c);
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        let strip = c
            .districts
            .iter()
            .find(|d| d.theme == Theme::NeonStrip)
            .unwrap();
        let rect = strip.bounds.grow(10);
        let tick = w.tick() + 30;
        let movers = w.visible(rect, tick);
        assert!(!movers.is_empty(), "the Strip has movers at eleven");
        let mut found_person = false;
        for m in &movers {
            assert!(rect.contains(m.pos), "{:?} inside the rect", m.pos);
            if let MoverKind::Walker { id, .. } = m.kind {
                let p = &w.people()[id.0 as usize];
                match &p.whereabouts {
                    Whereabouts::At { pos, .. } => assert_eq!(*pos, m.pos),
                    Whereabouts::Travelling(trip) => {
                        assert!(trip.legs.iter().any(|l| l.path.contains(&m.pos)))
                    }
                    Whereabouts::Inside(_) => panic!("inside people are not drawn"),
                }
                let there = w.person_at(m.pos, tick).expect("someone stands there");
                assert!(
                    movers.iter().any(|o| o.pos == m.pos
                        && matches!(o.kind, MoverKind::Walker { id: oid, .. } if oid == there)),
                    "person_at finds one of the walkers drawn on {:?}",
                    m.pos
                );
                found_person = true;
            }
        }
        assert!(found_person);
        let far = Rect::new(0, 0, 1, 1);
        assert!(w.visible(far, tick).iter().all(|m| far.contains(m.pos)));
    }

    #[test]
    fn the_same_seed_gives_the_same_day() {
        let c = city();
        let mut a = world(&c);
        let mut b = world(&c);
        run_to(&mut a, &c, tick_at(12 * 60, 1));
        run_to(&mut b, &c, tick_at(12 * 60, 1));
        assert_eq!(a.people(), b.people());
        assert_eq!(a.cars(), b.cars());
    }
}
