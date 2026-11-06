//! The world and its minute: who leaves, who arrives, who stands where, and what the
//! player can see and ask.

use crate::commute;
use crate::content::{DAY, Leisure, People};
use crate::paths::{drive_path, walk_cost, walk_path};
use crate::people::{
    Activity, Car, CarId, Dest, Goal, Leg, Person, PersonId, Population, Segment, SimParams, Trip,
    Whereabouts, Work, goal_at, seed_population,
};
use crate::places::Places;
use citygen::world::{BuildingKind, City, StratumKind, Theme};
use citygen::{BuildingId, FlavorPack, Hue, Pos, Rect, Size, Terrain};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Ticks in a game minute, as the HUD's clock keeps it.
pub const TICKS_PER_MINUTE: u64 = 90;
/// Ticks a walker spends per tile: ten tiles a game minute, so a commute keeps
/// someone on the street for minutes rather than seconds.
pub const WALK_TICKS: u64 = 9;
/// Ticks a car spends per tile: three times a walker.
pub const DRIVE_TICKS: u64 = 3;
/// A trip longer than this, on foot, is driven when there is a car; on a small map,
/// one longer than half its short side (`drive_from`).
pub const DRIVE_FROM: i32 = 60;

/// How long a trip on a map of `size` is driven from (when there is a car):
/// `DRIVE_FROM`, or half the map's short side if that's shorter, so a small city drives
/// its longer trips as a big one does.
pub fn drive_from(size: Size) -> i32 {
    let short = size.w.min(size.h) as i32;
    DRIVE_FROM.min(short / 2)
}

/// The profession that never leaves town: the city's jobs go through its fixers.
const FIXER: &str = "fixer";

/// A trip someone wants; `path_some` turns it into legs.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TripRequest {
    pub who: PersonId,
    pub dest: Dest,
    pub goal: Goal,
}

/// What the player can see of someone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mover {
    pub pos: Pos,
    pub kind: MoverKind,
}

/// What talking to someone, or inspecting them, shows.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Card {
    pub name: String,
    pub profession: String,
    pub doing: String,
    /// What comes next: an arrival, the next thing on the schedule, or nothing.
    pub next: String,
    pub lives: Option<String>,
    pub works: Option<String>,
    pub car: Option<String>,
    pub line: String,
    pub relation: Option<String>,
}

/// Someone the player swapped places with: drawn on `to` until the player moves on,
/// then back on `back` (a spot) or on their path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Displaced {
    /// The player's old tile, where they stand meanwhile.
    to: Pos,
    /// The tile the player took from them.
    from: Pos,
    /// A spot to return to; a walker just resumes their path.
    back: Option<Pos>,
}

/// Why someone is inside a building.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OccupantRole {
    Working,
    Living,
    Guest,
    Visiting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Occupant {
    pub id: PersonId,
    pub role: OccupantRole,
}

/// A client and a sex worker inside a hotel until a minute.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Date {
    client: PersonId,
    worker: PersonId,
    hotel: BuildingId,
    until: u64,
    beat: Pos,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct World {
    people: Vec<Person>,
    cars: Vec<Car>,
    households: Vec<Vec<PersonId>>,
    #[serde(skip)]
    places: Places,
    #[serde(skip)]
    labels: HashMap<String, String>,
    #[serde(skip)]
    night_lines: HashMap<String, String>,
    pending: VecDeque<TripRequest>,
    /// The last tick advanced to.
    tick: u64,
    /// The last minute stepped, counted from tick 0.
    minute: u64,
    start_minute: u16,
    open: BTreeSet<BuildingId>,
    chats: Vec<(Pos, u64)>,
    dates: Vec<Date>,
    rng: ChaCha8Rng,
    /// A tile nobody may stand on: the player's. Walkers are held a tile back.
    blocker: Option<Pos>,
    displaced: HashMap<PersonId, Displaced>,
    /// What someone was doing before a chat, to take up again after.
    chat_resume: HashMap<PersonId, Activity>,
    /// A goal someone has finished with early (a client after a date); the schedule
    /// does not send them back out for it.
    satisfied: HashMap<PersonId, Goal>,
    /// People who have been killed: they stay in `people()` but are never placed.
    dead: BTreeSet<PersonId>,
    /// People held at work: the schedule does not send them home.
    held: BTreeSet<PersonId>,
    /// People who never go off the map, whom the story has cast: not saved, made again
    /// from the cast whenever a run is reopened (`keep_in_town`).
    #[serde(skip)]
    in_town: BTreeSet<PersonId>,
}

fn labels_of(people: &People) -> HashMap<String, String> {
    people
        .professions
        .iter()
        .map(|p| (p.id.clone(), p.label.clone()))
        .collect()
}

fn night_lines_of(people: &People) -> HashMap<String, String> {
    people
        .professions
        .iter()
        .filter_map(|p| p.night_line.clone().map(|l| (p.id.clone(), l)))
        .collect()
}

/// The minute of the day a schedule's work ends: when the next of its segments begins.
fn knock_off(schedule: &[Segment]) -> Option<u16> {
    let work = schedule.iter().find(|s| s.goal == Goal::Work)?.at;
    schedule
        .iter()
        .filter(|s| s.goal != Goal::Work)
        .min_by_key(|s| (s.at + DAY - work) % DAY)
        .map(|s| s.at)
}

impl World {
    /// The city's people at `start_minute` of the day, placed where their schedules say.
    pub fn seed(
        city: &City,
        people: &People,
        params: &SimParams,
        start_minute: u16,
        tick: u64,
    ) -> World {
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
            labels: labels_of(people),
            night_lines: night_lines_of(people),
            pending: VecDeque::new(),
            tick,
            minute: tick / TICKS_PER_MINUTE,
            start_minute,
            open: BTreeSet::new(),
            chats: Vec::new(),
            dates: Vec::new(),
            rng,
            blocker: None,
            displaced: HashMap::new(),
            chat_resume: HashMap::new(),
            satisfied: HashMap::new(),
            dead: BTreeSet::new(),
            held: BTreeSet::new(),
            in_town: BTreeSet::new(),
        };
        w.place_everyone(city);
        w.refresh_open();
        w
    }

    /// Refills what a saved world leaves out (the places, the profession labels and night
    /// lines), as `seed` builds them from the city and the people.
    pub fn rebuild(&mut self, city: &City, people: &People) {
        self.places = Places::derive(city);
        self.labels = labels_of(people);
        self.night_lines = night_lines_of(people);
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

    /// Stands a person on a tile, working, with no trip in flight: a test aid for
    /// putting someone beside the player (another crate's tests cannot see a `cfg(test)` item).
    pub fn park(&mut self, id: PersonId, pos: Pos) {
        self.displaced.remove(&id);
        let p = &mut self.people[id.0 as usize];
        p.whereabouts = Whereabouts::At {
            pos,
            activity: Activity::Working,
        };
    }

    /// Kills a person: they never move or show again, though they stay in `people()`.
    pub fn kill(&mut self, id: PersonId) {
        let i = id.0 as usize;
        if i >= self.people.len() || !self.dead.insert(id) {
            return;
        }
        self.displaced.remove(&id);
        self.pending.retain(|r| r.who != id);
        self.chat_resume.remove(&id);
        self.park_cars_of(i);
    }

    /// A trip cut short leaves its car at the kerb it was bound for.
    fn park_cars_of(&mut self, i: usize) {
        if let Whereabouts::Travelling(t) = &self.people[i].whereabouts {
            for l in &t.legs {
                if let Some(c) = l.car {
                    let car = &mut self.cars[c.0 as usize];
                    car.parked = true;
                    car.pos = car
                        .bound
                        .take()
                        .unwrap_or(*l.path.last().unwrap_or(&car.pos));
                }
            }
        }
    }

    /// Holds a person at work: they are put at their workplace on the job at once, and
    /// the schedule does not send them home until `release`. A date they are on ends.
    pub fn keep_at_work(&mut self, id: PersonId) {
        let i = id.0 as usize;
        if i >= self.people.len() || self.dead.contains(&id) {
            return;
        }
        let at = match self.people[i].work {
            Work::Building(b) => Whereabouts::Inside(b),
            Work::Spot(pos) => Whereabouts::At {
                pos,
                activity: Activity::Working,
            },
            Work::Roaming => return,
        };
        self.end_dates_of(id);
        self.park_cars_of(i);
        self.displaced.remove(&id);
        self.chat_resume.remove(&id);
        self.pending.retain(|r| r.who != id);
        self.satisfied.remove(&id);
        self.people[i].goal = Goal::Work;
        self.people[i].whereabouts = at;
        self.held.insert(id);
        self.refresh_open();
    }

    /// Ends any date this person is on: the other one comes out as a date's end has
    /// them, the worker back on the beat, the client at the hotel's door.
    fn end_dates_of(&mut self, id: PersonId) {
        let (ended, kept) = std::mem::take(&mut self.dates)
            .into_iter()
            .partition(|d| d.client == id || d.worker == id);
        self.dates = kept;
        for d in ended {
            if d.worker != id && !self.dead.contains(&d.worker) {
                self.people[d.worker.0 as usize].whereabouts = Whereabouts::At {
                    pos: d.beat,
                    activity: Activity::Working,
                };
            }
            if d.client != id && !self.dead.contains(&d.client) {
                let door = self.places.doors[&d.hotel][0];
                self.people[d.client.0 as usize].whereabouts = Whereabouts::At {
                    pos: door,
                    activity: Activity::Loitering,
                };
            }
        }
    }

    /// Lets a held person follow their schedule again.
    pub fn release(&mut self, id: PersonId) {
        self.held.remove(&id);
    }

    /// Keeps a person in town: they never go off the map, home or out for anything (the
    /// story has a part for them); off it now, they come back the next minute the sim
    /// runs. Not saved: the story marks its cast again on reopening.
    pub fn keep_in_town(&mut self, id: PersonId) {
        if (id.0 as usize) < self.people.len() {
            self.in_town.insert(id);
        }
    }

    /// Whether this person is held at work.
    pub fn is_held(&self, id: PersonId) -> bool {
        self.held.contains(&id)
    }

    /// Whether this person has been killed.
    pub fn is_dead(&self, id: PersonId) -> bool {
        self.dead.contains(&id)
    }

    /// Gives a person a new name.
    pub fn rename(&mut self, id: PersonId, name: &str) {
        if let Some(p) = self.people.get_mut(id.0 as usize) {
            p.name = name.to_string();
        }
    }

    /// The span this person works, from their own shift; none for a roamer.
    pub fn work_hours(&self, id: PersonId) -> Option<(u16, u16)> {
        let p = self.people.get(id.0 as usize)?;
        (p.work != Work::Roaming).then_some(p.shift)
    }

    /// The way in someone who comes in to work from off the map comes and goes by; none
    /// for anyone who stays in town (`commute`).
    pub fn comes_in_by(&self, id: PersonId) -> Option<Pos> {
        let i = id.0 as usize;
        if i >= self.people.len() || !self.commuter(i) {
            return None;
        }
        self.way_in(i)
    }

    /// Where this person is drawn on the street right now; none indoors.
    pub fn pos_of(&self, id: PersonId) -> Option<Pos> {
        self.position(id.0 as usize).map(|(p, _)| p)
    }

    /// Where a spot worker stands.
    pub fn spot_of(&self, id: PersonId) -> Option<Pos> {
        match self.people.get(id.0 as usize)?.work {
            Work::Spot(p) => Some(p),
            _ => None,
        }
    }

    /// Everyone inside a building now and what they are there for, by id.
    pub fn inside(&self, b: BuildingId) -> Vec<Occupant> {
        self.people
            .iter()
            .filter(|p| p.whereabouts == Whereabouts::Inside(b) && !self.dead.contains(&p.id))
            .map(|p| {
                let on_a_date = self
                    .dates
                    .iter()
                    .any(|d| d.hotel == b && (d.client == p.id || d.worker == p.id));
                let role = if on_a_date {
                    OccupantRole::Guest
                } else if p.work == Work::Building(b) && p.goal == Goal::Work {
                    OccupantRole::Working
                } else if p.home == Some(b) {
                    OccupantRole::Living
                } else {
                    OccupantRole::Visiting
                };
                Occupant { id: p.id, role }
            })
            .collect()
    }

    /// Buildings with someone working inside right now.
    pub fn open_buildings(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.open.iter().copied()
    }

    /// A building's street doors.
    pub fn doors(&self, building: BuildingId) -> &[Pos] {
        self.places
            .doors
            .get(&building)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// The tile the player stands on; no walker is placed there.
    pub fn set_blocker(&mut self, pos: Option<Pos>) {
        self.blocker = pos;
    }

    /// The player steps from `player_from` onto someone standing on `player_to`:
    /// they take the player's old tile until the player moves on. Returns false for
    /// someone inside a building.
    pub fn swap(&mut self, id: PersonId, player_from: Pos, player_to: Pos) -> bool {
        let i = id.0 as usize;
        let back = match &self.people[i].whereabouts {
            Whereabouts::Inside(_) | Whereabouts::Away => return false,
            Whereabouts::At { pos, .. } => Some(*pos),
            Whereabouts::Travelling(_) => None,
        };
        let back = self.displaced.get(&id).map(|d| d.back).unwrap_or(back);
        if back == Some(player_from) {
            self.displaced.remove(&id);
        } else {
            self.displaced.insert(
                id,
                Displaced {
                    to: player_from,
                    from: player_to,
                    back,
                },
            );
        }
        true
    }

    /// Displaced people drift back once the player has left the tile they took.
    fn settle(&mut self) {
        let blocker = self.blocker;
        let people = &mut self.people;
        self.displaced.retain(|id, d| {
            let p = &mut people[id.0 as usize];
            let still = blocker == Some(d.from) && blocker != Some(d.to);
            match (&mut p.whereabouts, d.back) {
                (Whereabouts::At { pos, .. }, Some(back)) => {
                    if !still {
                        *pos = back;
                    }
                    still
                }
                (Whereabouts::Travelling(_), None) => still,
                _ => false,
            }
        });
    }

    /// Everyone goes straight to where the clock says, no trips: the seed state.
    fn place_everyone(&mut self, city: &City) {
        let mod_ = self.minute_of_day();
        for i in 0..self.people.len() {
            let goal = self.due(i, mod_);
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
                Some(Dest::Away(_)) => Whereabouts::Away,
                None => self.people[i].whereabouts.clone(),
            };
        }
    }

    /// The way in `i` comes and goes by from off the map; none when no road leaves it.
    /// Someone with no home in the city who works a building comes by a way whose walk
    /// goes to its door (`Places::ways_to`); none if no way's does.
    fn way_in(&self, i: usize) -> Option<Pos> {
        let p = &self.people[i];
        let ways = match (p.home, p.work) {
            (None, Work::Building(b)) => self.places.ways_to(b),
            _ => &self.places.ways_in,
        };
        commute::way_in_of(ways, p.id)
    }

    /// The road `i` leaves the map by: none for someone the story keeps in town
    /// (`keep_in_town`), a fixer, or a map no road leaves.
    fn way_out(&self, i: usize) -> Option<Pos> {
        let p = &self.people[i];
        if p.profession == FIXER || self.in_town.contains(&p.id) {
            return None;
        }
        self.way_in(i)
    }

    /// Whether `i` comes in to work from off the map: no home in the city, work that
    /// isn't a spot of the street's, and a road out they may take (one that walks to
    /// their work).
    fn commuter(&self, i: usize) -> bool {
        let p = &self.people[i];
        p.home.is_none() && !matches!(p.work, Work::Spot(_)) && self.way_out(i).is_some()
    }

    /// Minutes the walk to work from off the map takes `i`, who comes in to a building;
    /// none for anyone else.
    fn walk_in(&self, i: usize) -> Option<u16> {
        let Work::Building(b) = self.people[i].work else {
            return None;
        };
        if !self.commuter(i) {
            return None;
        }
        let way = self.way_in(i)?;
        let at = self.places.door_from(b, way)?;
        Some(commute::walk_minutes(way, at))
    }

    /// Whether `i`, who comes in to a building, is on their break at `minute` of the day:
    /// one in a few, the break wholly within their day's work (`commute::lunch_of`).
    fn at_lunch(&self, i: usize, minute: u16) -> bool {
        let p = &self.people[i];
        if !matches!(p.work, Work::Building(_)) {
            return false;
        }
        let Some((from, long)) = commute::lunch_of(p.id) else {
            return false;
        };
        let to = (from + long) % DAY;
        commute::between(minute, from, to)
            && goal_at(&p.schedule, from) == Goal::Work
            && goal_at(&p.schedule, to) == Goal::Work
    }

    /// Where `i`, who comes in to work, has a drink after it at `minute` of the day, if
    /// they're one who stays on (`commute::drink_until`), knocked off by day and not
    /// gone yet: their own leisure if the city has it, else the first it has; none in a
    /// city without a bar, a noodle bar or the Strip.
    fn drink(&self, i: usize, minute: u16) -> Option<Leisure> {
        let p = &self.people[i];
        let until = commute::drink_until(p.id)?;
        let off = knock_off(&p.schedule)?;
        let (early, late) = commute::DRINK_AFTER;
        if !(early..late).contains(&off) || !commute::between(minute, off, until) {
            return None;
        }
        let drinks = &self.places.drinks;
        let own = p.schedule.iter().find_map(|s| match s.goal {
            Goal::Leisure(l) => Some(l),
            _ => None,
        });
        own.filter(|l| drinks.contains(l))
            .or_else(|| drinks.first().copied())
    }

    /// `goal` as someone who lives in town keeps it: never out of town for a drink. One
    /// the city has no place for is had at the first it has, else they stay home.
    fn in_town(&self, goal: Goal) -> Goal {
        let drinks = &self.places.drinks;
        match goal {
            Goal::Leisure(l) if !drinks.contains(&l) => {
                drinks.first().map_or(Goal::Home, |l| Goal::Leisure(*l))
            }
            _ => goal,
        }
    }

    /// The goal `i` keeps at `minute` of the day: their schedule's, but someone who comes
    /// in from off the map sets off for work as much earlier as the walk in takes, may
    /// take a break at midday and may stay on in town for a drink after; and someone who
    /// lives in town has their drink in town (`in_town`).
    fn due(&self, i: usize, minute: u16) -> Goal {
        let schedule = &self.people[i].schedule;
        let now = goal_at(schedule, minute);
        if !self.commuter(i) {
            return self.in_town(now);
        }
        if now == Goal::Work {
            return if self.at_lunch(i, minute) {
                Goal::Lunch
            } else {
                now
            };
        }
        if let Some(lead) = self.walk_in(i)
            && goal_at(schedule, (minute + lead) % DAY) == Goal::Work
        {
            return Goal::Work;
        }
        self.drink(i, minute).map_or(now, Goal::Leisure)
    }

    /// Where a goal takes a person: their home, their work, their bar, the Strip, a break;
    /// off the map by their way in when that's where it is (a home out of town) or the
    /// city has no such place. A spot worker with no home keeps to their spot.
    fn dest_for(&mut self, city: &City, i: usize, goal: Goal) -> Option<Dest> {
        let away = self.way_out(i).map(Dest::Away);
        let p = &self.people[i];
        let here = match goal {
            Goal::Home => match (p.home, p.work) {
                (Some(h), _) => Some(Dest::Building(h)),
                (None, Work::Spot(s)) => Some(Dest::Spot(s)),
                (None, _) => away,
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
            // a drink their day wouldn't have had: a beat by who they are, the city's dice
            // left alone
            Goal::Leisure(Leisure::Strip) if goal_at(&p.schedule, self.minute_of_day()) != goal => {
                self.beat_for(i)
            }
            Goal::Leisure(Leisure::Strip) => self.a_beat(),
            Goal::Lunch => self.lunch_place(city, i),
        };
        match goal {
            Goal::Lunch => here,
            _ => here.or(away),
        }
    }

    /// Brings `i` back from off the map for `goal`, for they have no road out now: to
    /// where it takes them, else home, else work; walking in by their way when its walk
    /// goes there, else put there at once. Once: not again while that walk is asked for.
    fn come_back(&mut self, city: &City, i: usize, goal: Goal) {
        let who = self.people[i].id;
        if self.pending.iter().any(|r| r.who == who) {
            return;
        }
        let Some(dest) = [goal, Goal::Home, Goal::Work]
            .into_iter()
            .find_map(|g| self.dest_for(city, i, g))
        else {
            return;
        };
        self.people[i].goal = goal;
        let walks = &self.places.walks;
        let walk_in = self.way_in(i).is_some_and(|way| match dest {
            Dest::Building(b) => self
                .places
                .door_from(b, way)
                .is_some_and(|d| walks.reaches(way, d)),
            Dest::Spot(s) => walks.reaches(way, s),
            Dest::Away(_) => false,
        });
        if walk_in {
            self.pending.push_back(TripRequest { who, dest, goal });
        } else {
            self.arrive(i, dest, goal);
        }
    }

    /// Where `i` goes on their break: somewhere to eat within reach of work, else a spot
    /// along the street near it, by who they are.
    fn lunch_place(&self, city: &City, i: usize) -> Option<Dest> {
        let p = &self.people[i];
        let Work::Building(b) = p.work else {
            return None;
        };
        let door = *self.places.doors.get(&b)?.first()?;
        let food = self
            .places
            .food
            .iter()
            .filter(|(f, _)| *f != b)
            .map(|(f, d)| (d.manhattan(door), f.index(), *f))
            .filter(|(d, _, _)| *d <= commute::LUNCH_REACH)
            .min();
        if let Some((_, _, f)) = food {
            return Some(Dest::Building(f));
        }
        let (near, far) = commute::LUNCH_SPOT;
        let spots: Vec<Pos> = Rect::new(door.x - far, door.y - far, 2 * far + 1, 2 * far + 1)
            .iter()
            .filter(|q| (near..=far).contains(&q.manhattan(door)))
            .filter(|q| {
                matches!(
                    city.tile(StratumKind::Street, *q).map(|t| t.terrain),
                    Some(Terrain::Sidewalk | Terrain::Plaza | Terrain::Walkway)
                ) && walk_cost(city, *q, *q) == Some(1)
            })
            .collect();
        (!spots.is_empty()).then(|| Dest::Spot(spots[commute::pick_for(p.id, 0x5B07, spots.len())]))
    }

    /// A spot on the Strip's beats by who `i` is; none without a beat.
    fn beat_for(&self, i: usize) -> Option<Dest> {
        let id = self.people[i].id;
        let mut districts: Vec<_> = self.places.beats.keys().copied().collect();
        districts.sort_by_key(|d| d.index());
        let d = *districts.get(commute::pick_for(id, 0xBEA7, districts.len()))?;
        let beats = &self.places.beats[&d];
        Some(Dest::Spot(
            beats[commute::pick_for(id, 0xBEA8, beats.len())],
        ))
    }

    /// A spot on the Strip's beats, at random; none without a beat.
    fn a_beat(&mut self) -> Option<Dest> {
        let mut districts: Vec<_> = self.places.beats.keys().copied().collect();
        districts.sort_by_key(|d| d.index());
        let d = *districts.get(self.rng.random_range(0..districts.len().max(1)))?;
        let beats = &self.places.beats[&d];
        Some(Dest::Spot(beats[self.rng.random_range(0..beats.len())]))
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
            Whereabouts::Away => self.way_in(i),
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

    /// Every tile a car stands on or is bound for: a car in flight keeps its kerb
    /// until it leaves and holds its destination from the moment it is dispatched.
    fn occupied_tiles(&self) -> HashSet<Pos> {
        self.cars
            .iter()
            .flat_map(|c| std::iter::once(c.pos).chain(c.bound))
            .collect()
    }

    /// Turns up to `budget` requests into trips, dropping any with no path.
    pub fn path_some(&mut self, city: &City, budget: usize) {
        for _ in 0..budget {
            let Some(req) = self.pending.pop_front() else {
                break;
            };
            if self.dead.contains(&req.who) {
                continue;
            }
            let i = req.who.0 as usize;
            let Some(from) = self.anchor(i) else { continue };
            let to = match req.dest {
                Dest::Building(b) => {
                    let Some(door) = self.places.door_from(b, from) else {
                        continue;
                    };
                    door
                }
                Dest::Spot(s) | Dest::Away(s) => s,
            };
            if from == to {
                self.arrive(i, req.dest, req.goal);
                continue;
            }
            let mut legs: Vec<Leg> = Vec::new();
            // off the map and back is on foot: the car stays where it's parked
            let off_the_map = matches!(req.dest, Dest::Away(_))
                || self.people[i].whereabouts == Whereabouts::Away;
            let car = self.people[i]
                .car
                .filter(|c| self.cars[c.0 as usize].parked && !off_the_map);
            if let Some(car_id) = car
                && from.manhattan(to) > drive_from(city.size)
            {
                let car_pos = self.cars[car_id.0 as usize].pos;
                let mut occupied = self.occupied_tiles();
                occupied.remove(&car_pos);
                if let Some(kerb) = self.free_kerb_near(city, to, &occupied)
                    && kerb != car_pos
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
                    car.bound = Some(kerb);
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
            Dest::Away(_) => Whereabouts::Away,
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

    /// Where a person is at `tick`, if on the street. A walker due on the blocked
    /// tile waits one tile back; a loiterer on it steps out of sight for the moment.
    fn position(&self, i: usize) -> Option<(Pos, Option<CarId>)> {
        if self.dead.contains(&self.people[i].id) {
            return None;
        }
        let tick = self.tick;
        if let Some(d) = self.displaced.get(&self.people[i].id) {
            return Some((d.to, None));
        }
        match &self.people[i].whereabouts {
            Whereabouts::Inside(_) | Whereabouts::Away => None,
            Whereabouts::At { pos, .. } => {
                if self.blocker == Some(*pos) {
                    None
                } else {
                    Some((*pos, None))
                }
            }
            Whereabouts::Travelling(t) => {
                let elapsed = tick.saturating_sub(t.depart);
                let (pos, car) = Self::trip_pos(t, elapsed);
                if car.is_none() && self.blocker == Some(pos) {
                    let (back, car) = Self::trip_pos(t, elapsed.saturating_sub(WALK_TICKS));
                    if back == pos { None } else { Some((back, car)) }
                } else {
                    Some((pos, car))
                }
            }
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
        self.settle();
    }

    fn refresh_open(&mut self) {
        self.open = self
            .people
            .iter()
            .filter(|p| p.goal == Goal::Work && !self.dead.contains(&p.id))
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
            if self.dead.contains(&PersonId(i as u32)) {
                continue;
            }
            if let Whereabouts::Travelling(t) = &self.people[i].whereabouts
                && now.saturating_sub(t.depart) >= Self::trip_ticks(t)
            {
                let (dest, goal) = (t.dest, t.goal);
                for l in &t.legs {
                    if let Some(c) = l.car {
                        let car = &mut self.cars[c.0 as usize];
                        car.parked = true;
                        car.pos = car
                            .bound
                            .take()
                            .unwrap_or(*l.path.last().unwrap_or(&car.pos));
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
            if !self.dead.contains(&d.worker) {
                self.people[d.worker.0 as usize].whereabouts = Whereabouts::At {
                    pos: d.beat,
                    activity: Activity::Working,
                };
            }
            if self.dead.contains(&d.client) {
                continue;
            }
            let door = self.places.doors[&d.hotel][0];
            let ci = d.client.0 as usize;
            self.people[ci].whereabouts = Whereabouts::At {
                pos: door,
                activity: Activity::Loitering,
            };
            // Done for the night: home, and the schedule's Strip does not pull them
            // back out.
            self.people[ci].goal = Goal::Home;
            self.satisfied
                .insert(d.client, Goal::Leisure(Leisure::Strip));
            if let Some(dest) = self.dest_for(city, ci, Goal::Home) {
                self.pending.push_back(TripRequest {
                    who: d.client,
                    dest,
                    goal: Goal::Home,
                });
            }
        }
        // Departures: the schedule changed its mind, and the person is free to go.
        for i in 0..self.people.len() {
            if self.dead.contains(&PersonId(i as u32)) {
                continue;
            }
            if matches!(self.people[i].whereabouts, Whereabouts::Travelling(_)) {
                continue;
            }
            if self.held.contains(&PersonId(i as u32)) {
                continue;
            }
            if self
                .dates
                .iter()
                .any(|d| d.client.0 as usize == i || d.worker.0 as usize == i)
            {
                continue;
            }
            let goal = self.due(i, mod_);
            // off the map with no road out now (kept in town, or no way in walks to their
            // work): back in
            if self.people[i].whereabouts == Whereabouts::Away && self.way_out(i).is_none() {
                self.come_back(city, i, goal);
                continue;
            }
            if let Some(done) = self.satisfied.get(&PersonId(i as u32)) {
                if *done == goal {
                    continue;
                }
                self.satisfied.remove(&PersonId(i as u32));
            }
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
            if self.dead.contains(&PersonId(i as u32)) {
                continue;
            }
            let anchor = match self.people[i].work {
                Work::Spot(a) => Some(a),
                _ => None,
            };
            let roll = self.rng.random::<f32>();
            let pick = self.rng.random_range(0..5usize);
            if let Whereabouts::At { pos, activity } = &mut self.people[i].whereabouts {
                if *activity == Activity::Chatting {
                    *activity = self
                        .chat_resume
                        .remove(&PersonId(i as u32))
                        .unwrap_or(Activity::Working);
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
                        if *activity != Activity::Chatting {
                            self.chat_resume.insert(PersonId(i as u32), *activity);
                        }
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
            .filter(|(_, p)| p.profession == "sex_worker" && !self.dead.contains(&p.id))
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
            .filter(|(_, p)| {
                !self.dead.contains(&p.id) && p.client && p.goal == Goal::Leisure(Leisure::Strip)
            })
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

    /// Everyone and every car inside `bubble` now, with chat markers.
    pub fn visible(&self, bubble: Rect) -> Vec<Mover> {
        let mut out = Vec::new();
        for (i, p) in self.people.iter().enumerate() {
            let Some((pos, None)) = self.position(i) else {
                continue;
            };
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
        for car in &self.cars {
            let (pos, moving) = self.car_spot(car.id);
            let parked = !moving;
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

    /// The walker standing on `pos` now, if any.
    pub fn person_at(&self, pos: Pos) -> Option<PersonId> {
        (0..self.people.len()).find_map(|i| match self.position(i) {
            Some((p, None)) if p == pos => Some(self.people[i].id),
            _ => None,
        })
    }

    /// Where a car is drawn and whether it is moving: parked at its kerb, waiting
    /// there while its owner walks to it, on the road, or at the kerb it was bound
    /// for once the owner has got out.
    pub fn car_spot(&self, id: CarId) -> (Pos, bool) {
        let car = &self.cars[id.0 as usize];
        if car.parked {
            return (car.pos, false);
        }
        let bound = car.bound.unwrap_or(car.pos);
        let Whereabouts::Travelling(t) = &self.people[car.owner.0 as usize].whereabouts else {
            return (bound, false);
        };
        let elapsed = self.tick.saturating_sub(t.depart);
        let mut start = 0;
        for l in &t.legs {
            let per = if l.car.is_some() {
                DRIVE_TICKS
            } else {
                WALK_TICKS
            };
            let span = l.path.len().saturating_sub(1) as u64 * per;
            if l.car == Some(id) {
                return if elapsed < start {
                    (car.pos, false)
                } else if elapsed < start + span {
                    (Self::trip_pos(t, elapsed).0, true)
                } else {
                    (bound, false)
                };
            }
            start += span;
        }
        (bound, false)
    }

    /// The car on `pos` now, parked or driving.
    pub fn car_at(&self, pos: Pos) -> Option<CarId> {
        self.cars
            .iter()
            .find(|c| self.car_spot(c.id).0 == pos)
            .map(|c| c.id)
    }

    fn label(&self, profession: &str) -> String {
        self.labels
            .get(profession)
            .cloned()
            .unwrap_or_else(|| profession.replace('_', " "))
    }

    fn goal_name(goal: Goal, p: &Person) -> &'static str {
        match goal {
            Goal::Home => "home",
            Goal::Work => match p.work {
                Work::Roaming => "deliveries",
                _ => "work",
            },
            Goal::Leisure(Leisure::Bar) => "the bar",
            Goal::Leisure(Leisure::NoodleBar) => "noodles",
            Goal::Leisure(Leisure::Strip) => "the Strip",
            Goal::Lunch => "a break",
        }
    }

    /// Where a destination reads as: a building by name, or the kind of spot.
    fn dest_name(city: &City, p: &Person, dest: Dest) -> String {
        match dest {
            Dest::Building(b) => Self::place_name(city, b),
            Dest::Away(_) => "the edge of town".to_string(),
            Dest::Spot(_) => match p.profession.as_str() {
                "sex_worker" => "the beat",
                "fixer" => "the corner",
                "ganger" => "the block",
                "door_muscle" => "the door",
                _ => "the street",
            }
            .to_string(),
        }
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
                // The doc keeps to the back; the street sees the front.
                BuildingKind::Clinic => "in the back room".to_string(),
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
            (Whereabouts::At { .. }, Goal::Lunch) => "on a break".to_string(),
            (Whereabouts::At { .. }, _) => "hanging around".to_string(),
            (Whereabouts::Away, _) => "out of town".to_string(),
            (Whereabouts::Travelling(t), _) => match t.goal {
                Goal::Home => "heading home".to_string(),
                Goal::Work => match p.work {
                    Work::Roaming => "on a delivery".to_string(),
                    _ => "heading to work".to_string(),
                },
                Goal::Leisure(Leisure::Bar) => "off to the bar".to_string(),
                Goal::Leisure(Leisure::NoodleBar) => "off for noodles".to_string(),
                Goal::Leisure(Leisure::Strip) => "heading for the Strip".to_string(),
                Goal::Lunch => "out on a break".to_string(),
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
        let hhmm = |m: u16| format!("{:02}:{:02}", m / 60, m % 60);
        let next = if let Some(d) = self.dates.iter().find(|d| d.client == id || d.worker == id) {
            let left = d.until.saturating_sub(self.minute).max(1);
            format!("out of {} in {left} min", Self::place_name(city, d.hotel))
        } else {
            match &p.whereabouts {
                Whereabouts::Travelling(t) => {
                    let left =
                        Self::trip_ticks(t).saturating_sub(self.tick.saturating_sub(t.depart));
                    let mins = left.div_ceil(TICKS_PER_MINUTE).max(1);
                    format!(
                        "arrives at {} in {mins} min",
                        Self::dest_name(city, p, t.dest)
                    )
                }
                _ => {
                    let mod_ = self.minute_of_day();
                    let after = p
                        .schedule
                        .iter()
                        .filter(|s| s.at > mod_)
                        .min_by_key(|s| s.at)
                        .or_else(|| p.schedule.iter().min_by_key(|s| s.at));
                    match after {
                        Some(s) if p.schedule.len() > 1 => {
                            format!("{} at {}", Self::goal_name(s.goal, p), hhmm(s.at))
                        }
                        _ => "stays put".to_string(),
                    }
                }
            }
        };
        let lives = Some(match p.home {
            Some(h) => Self::place_name(city, h),
            None if self.commuter(id.0 as usize) => "out of town".to_string(),
            None => "the street".to_string(),
        });
        let works = Some(match p.work {
            Work::Building(b) => Self::place_name(city, b),
            Work::Spot(_) => Self::dest_name(city, p, Dest::Spot(Pos::new(0, 0))),
            Work::Roaming => "the whole city".to_string(),
        });
        let car = p.car.map(|c| {
            let car = &self.cars[c.0 as usize];
            if car.parked {
                format!("parked at {},{}", car.pos.x, car.pos.y)
            } else {
                "on the road".to_string()
            }
        });
        Card {
            name: p.name.clone(),
            profession: self.label(&p.profession),
            doing,
            next,
            lives,
            works,
            car,
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
    use crate::people::Segment;
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
        World::seed(
            c,
            &People::load().unwrap(),
            &SimParams::default(),
            22 * 60,
            0,
        )
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

    /// A ripperdoc and her clinic.
    fn a_doc(w: &World) -> (PersonId, BuildingId) {
        let p = one(w, |p| {
            p.profession == "ripperdoc" && matches!(p.work, Work::Building(_))
        });
        let Work::Building(b) = p.work else {
            unreachable!()
        };
        (p.id, b)
    }

    fn working_in(w: &World, b: BuildingId, id: PersonId) -> bool {
        w.inside(b)
            .iter()
            .any(|o| o.id == id && o.role == OccupantRole::Working)
    }

    #[test]
    fn a_held_worker_stays_at_work_out_of_hours() {
        let c = city();
        let mut w = world(&c);
        let (id, b) = a_doc(&w);
        run_to(&mut w, &c, tick_at(10 * 60, 1));
        assert!(!working_in(&w, b, id), "off at ten");
        w.keep_at_work(id);
        assert!(w.is_held(id));
        assert!(working_in(&w, b, id));
        run_to(&mut w, &c, tick_at(16 * 60, 1));
        assert!(working_in(&w, b, id), "still at work six hours on");
        assert!(w.is_open(b));
    }

    #[test]
    fn release_sends_them_where_the_schedule_says() {
        let c = city();
        let mut w = world(&c);
        let (id, b) = a_doc(&w);
        run_to(&mut w, &c, tick_at(10 * 60, 1));
        w.keep_at_work(id);
        w.release(id);
        assert!(!w.is_held(id));
        run_to(&mut w, &c, tick_at(12 * 60, 1));
        assert!(!w.inside(b).iter().any(|o| o.id == id));
    }

    #[test]
    fn holding_someone_on_a_date_ends_it() {
        let c = city();
        let mut w = world(&c);
        let (id, b) = a_doc(&w);
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        let (wi, beat) = (0..w.people.len())
            .find_map(|i| match w.people[i].whereabouts {
                Whereabouts::At {
                    pos,
                    activity: Activity::Working,
                } if w.people[i].profession == "sex_worker" => Some((i, pos)),
                _ => None,
            })
            .expect("someone on the beat");
        let hotel = *w.places.doors.keys().find(|h| **h != b).unwrap();
        let worker = PersonId(wi as u32);
        // The doc on a date in a hotel, ten minutes to go.
        for who in [id, worker] {
            w.people[who.0 as usize].whereabouts = Whereabouts::Inside(hotel);
        }
        w.dates.push(Date {
            client: id,
            worker,
            hotel,
            until: w.minute + 10,
            beat,
        });
        w.keep_at_work(id);
        assert!(w.dates.is_empty(), "the date is over");
        assert_eq!(
            w.people[wi].whereabouts,
            Whereabouts::At {
                pos: beat,
                activity: Activity::Working
            },
            "back on the beat"
        );
        let later = w.tick() + 20 * TICKS_PER_MINUTE;
        run_to(&mut w, &c, later);
        assert!(working_in(&w, b, id), "never walked out of the hotel");
    }

    #[test]
    fn a_new_world_holds_nobody() {
        let c = city();
        let w = world(&c);
        assert!(w.people().iter().all(|p| !w.is_held(p.id)));
    }

    /// One doc to a clinic, in the back room by night, gone by day.
    #[test]
    fn a_clinic_has_one_doc_in_the_back_room_by_night() {
        let c = city();
        let mut w = world(&c);
        let clinics: Vec<_> = c
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Clinic)
            .map(|b| b.id)
            .collect();
        assert!(!clinics.is_empty(), "the test city has a clinic");
        for b in &clinics {
            let docs = w
                .people()
                .iter()
                .filter(|p| p.profession == "ripperdoc" && p.work == Work::Building(*b))
                .count();
            assert_eq!(docs, 1, "{b:?}");
        }
        let id = one(&w, |p| p.profession == "ripperdoc").id;
        let Work::Building(clinic) = w.people()[id.0 as usize].work else {
            unreachable!()
        };
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        assert_eq!(
            w.people()[id.0 as usize].whereabouts,
            Whereabouts::Inside(clinic)
        );
        assert_eq!(w.card(id, &c).doing, "in the back room");
        run_to(&mut w, &c, tick_at(12 * 60, 1));
        assert_ne!(
            w.people()[id.0 as usize].whereabouts,
            Whereabouts::Inside(clinic)
        );
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
        assert!(
            card.next.contains(" at "),
            "the next thing on the schedule: {}",
            card.next
        );
        assert!(card.lives.is_some() && card.works.is_some());
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
        let mut waited = false;
        let mut t = w.tick();
        let end = tick_at(22 * 60, 1);
        while t < end {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            // Cars that are not moving never share a tile, all day long.
            let mut seen = HashSet::new();
            for car in w.cars() {
                let (spot, moving) = w.car_spot(car.id);
                if !moving {
                    assert!(
                        seen.insert(spot),
                        "two cars standing on {spot:?} at tick {t}"
                    );
                }
            }
            if let Whereabouts::Travelling(trip) = &w.people()[driver.id.0 as usize].whereabouts
                && trip.legs.iter().any(|l| l.car.is_some())
            {
                drove = true;
                // While the owner walks to the car, it waits where it was left.
                let car_id = driver.car.unwrap();
                let walk_to_car = trip.legs[0].path.len().saturating_sub(1) as u64 * WALK_TICKS;
                if t.saturating_sub(trip.depart) < walk_to_car {
                    let origin = *trip.legs[0].path.last().unwrap();
                    assert_eq!(
                        w.car_spot(car_id),
                        (origin, false),
                        "the car waits at its kerb"
                    );
                    assert_eq!(w.car_at(origin), Some(car_id));
                    waited = true;
                }
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
        }
        assert!(drove, "a long commute with a car is driven");
        assert!(waited, "the walk to the car was seen");
        let p = &w.people()[driver.id.0 as usize];
        assert_eq!(
            p.whereabouts,
            Whereabouts::Inside(p.home.unwrap()),
            "home by ten at night"
        );
        let car = &w.cars()[driver.car.unwrap().0 as usize];
        assert!(car.parked);
        assert!(places.is_kerb(car.pos) || places.lots.iter().any(|(_, t)| t.contains(&car.pos)));
        assert!(
            car.pos.manhattan(places.doors[&p.home.unwrap()][0]) <= 40,
            "parked near home: {:?} (work {:?})",
            car.pos,
            work
        );
    }

    #[test]
    fn a_small_city_drives_its_longer_trips() {
        // driven past 60 cells on a map as big as a home city's (none has a short side
        // under 160), past half the short side on a smaller one
        let size = |w, h| Size { w, h };
        assert_eq!(drive_from(size(336, 176)), DRIVE_FROM);
        assert_eq!(drive_from(size(288, 160)), DRIVE_FROM);
        assert_eq!(drive_from(size(136, 80)), 40);
        assert_eq!(drive_from(size(60, 120)), 30);
        // on the test city, 120 by 60, a commute with a car of 40 to 60 cells is driven
        let c = city();
        let mut w = world(&c);
        let places = Places::derive(&c);
        let driver = one(&w, |p| {
            p.car.is_some()
                && matches!((p.home, p.work), (Some(h), Work::Building(b))
                    if (40..=60).contains(&places.doors[&h][0].manhattan(places.doors[&b][0])))
        })
        .clone();
        let mut drove = false;
        let mut t = w.tick();
        while t < tick_at(22 * 60, 1) {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            drove |= matches!(&w.people()[driver.id.0 as usize].whereabouts,
                Whereabouts::Travelling(trip) if trip.legs.iter().any(|l| l.car.is_some()));
        }
        assert!(drove, "{} drives to work", driver.name);
    }

    #[test]
    fn cars_never_stack_on_the_default_city() {
        let c = Generator::run(GenParams::default()).unwrap();
        let mut w = world(&c);
        let mut t = w.tick();
        while t < tick_at(60, 1) {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            let mut seen = HashSet::new();
            for car in w.cars() {
                let (spot, moving) = w.car_spot(car.id);
                if !moving {
                    assert!(
                        seen.insert(spot),
                        "two cars standing on {spot:?} at tick {t}"
                    );
                }
            }
        }
    }

    #[test]
    fn door_muscle_stands_at_the_post_and_towers_close_at_night() {
        let c = city();
        let mut w = world(&c);
        let places = Places::derive(&c);
        let posts: HashSet<Pos> = places.posts.values().copied().collect();
        let muscle: Vec<&Person> = w
            .people()
            .iter()
            .filter(|p| p.profession == "door_muscle")
            .collect();
        assert!(!muscle.is_empty());
        for m in &muscle {
            let Work::Spot(p) = m.work else {
                panic!("{} works inside", m.name)
            };
            assert!(posts.contains(&p), "{} stands at a post", m.name);
            let night = m.shift_start == 1140;
            let kinds: Vec<BuildingKind> = places
                .posts
                .iter()
                .filter(|(_, q)| **q == p)
                .map(|(b, _)| c.buildings[b.index()].kind)
                .collect();
            assert!(
                kinds.iter().all(|k| if night {
                    *k == BuildingKind::Club
                } else {
                    matches!(k, BuildingKind::CorpTower | BuildingKind::Office)
                }),
                "{} ({}) at {:?}",
                m.name,
                if night { "night" } else { "day" },
                kinds
            );
        }
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        let on_the_door = |w: &World| {
            w.people()
                .iter()
                .filter(|p| p.profession == "door_muscle")
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
        assert!(on_the_door(&w) > 0, "muscle on the club doors at eleven");
        run_to(&mut w, &c, tick_at(4 * 60, 1));
        assert!(
            w.open_buildings()
                .all(|b| c.buildings[b.index()].kind != BuildingKind::CorpTower),
            "no tower is open at four in the morning"
        );
        run_to(&mut w, &c, tick_at(12 * 60, 1));
        assert!(on_the_door(&w) > 0, "muscle on the tower doors at noon");
    }

    #[test]
    fn a_chat_hands_back_the_activity() {
        let c = city();
        let mut w = world(&c);
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        // Two people whose schedule keeps them home till two, stood on one tile: one
        // in a queue, one idling.
        let stay: Vec<usize> = (0..w.people.len())
            .filter(|&i| {
                let p = &w.people[i];
                !p.client
                    && p.goal == Goal::Home
                    && (0..=180).all(|m| goal_at(&p.schedule, m) == Goal::Home)
                    && matches!(p.whereabouts, Whereabouts::Inside(_))
            })
            .take(2)
            .collect();
        let [a, b] = stay[..] else {
            panic!("two people at home")
        };
        let tile = *w
            .places
            .kerbs
            .iter()
            .find(|k| walk_cost(&c, k.step(citygen::Dir::N), k.step(citygen::Dir::N)) == Some(1))
            .map(|k| k.step(citygen::Dir::N))
            .as_ref()
            .expect("a sidewalk");
        w.people[a].whereabouts = Whereabouts::At {
            pos: tile,
            activity: Activity::Queueing,
        };
        w.people[b].whereabouts = Whereabouts::At {
            pos: tile,
            activity: Activity::Loitering,
        };
        let act = |w: &World, i: usize| match w.people[i].whereabouts {
            Whereabouts::At { activity, .. } => Some(activity),
            ref other => panic!("moved: {other:?}"),
        };
        let mut chatted = false;
        for _ in 0..120 {
            let t = w.tick() + TICKS_PER_MINUTE;
            w.advance(&c, t);
            if act(&w, a) == Some(Activity::Chatting) {
                assert_eq!(act(&w, b), Some(Activity::Chatting));
                chatted = true;
                continue;
            }
            if chatted {
                assert_eq!(
                    act(&w, a),
                    Some(Activity::Queueing),
                    "back in the queue after the chat"
                );
                assert_eq!(
                    act(&w, b),
                    Some(Activity::Loitering),
                    "idling again after the chat"
                );
                return;
            }
        }
        panic!("no chat in two hours on a shared tile");
    }

    #[test]
    fn a_client_goes_home_after_a_date() {
        let c = city();
        let mut w = world(&c);
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        let (wi, beat) = (0..w.people.len())
            .find_map(|i| match w.people[i].whereabouts {
                Whereabouts::At {
                    pos,
                    activity: Activity::Working,
                } if w.people[i].profession == "sex_worker" => Some((i, pos)),
                _ => None,
            })
            .expect("someone on the beat");
        let ci = (0..w.people.len())
            .find(|&i| w.people[i].client && w.people[i].home.is_some() && i != wi)
            .expect("a client with a home");
        w.people[ci].schedule = vec![Segment {
            at: 0,
            goal: Goal::Leisure(Leisure::Strip),
        }];
        w.people[ci].goal = Goal::Leisure(Leisure::Strip);
        w.people[ci].whereabouts = Whereabouts::At {
            pos: beat,
            activity: Activity::Loitering,
        };
        let mut dated = false;
        for _ in 0..40 {
            let t = w.tick() + TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            if w.dates.iter().any(|d| d.client.0 as usize == ci) {
                dated = true;
                break;
            }
        }
        assert!(dated, "a client beside a worker is taken in");
        for _ in 0..45 {
            let t = w.tick() + TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            if !w.dates.iter().any(|d| d.client.0 as usize == ci) {
                break;
            }
        }
        let client = &w.people[ci];
        assert_eq!(
            client.goal,
            Goal::Home,
            "done for the night: {:?}",
            client.whereabouts
        );
        assert!(
            matches!(&client.whereabouts, Whereabouts::Travelling(t) if t.goal == Goal::Home)
                || client.whereabouts == Whereabouts::Inside(client.home.unwrap()),
            "{:?}",
            client.whereabouts
        );
        let worker = &w.people[wi];
        assert!(
            matches!(
                worker.whereabouts,
                Whereabouts::At {
                    activity: Activity::Working,
                    ..
                }
            ) || matches!(worker.whereabouts, Whereabouts::Inside(_)),
            "{:?}",
            worker.whereabouts
        );
        for _ in 0..60 {
            let t = w.tick() + TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            assert_eq!(
                w.people[ci].goal,
                Goal::Home,
                "the schedule's Strip does not pull them back out"
            );
        }
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
        let movers = w.visible(rect);
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
                    Whereabouts::Away => panic!("people off the map are not drawn"),
                }
                let there = w.person_at(m.pos).expect("someone stands there");
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
        assert!(w.visible(far).iter().all(|m| far.contains(m.pos)));
    }

    #[test]
    fn a_bump_swaps_places_and_they_drift_back() {
        let c = city();
        let mut w = world(&c);
        run_to(&mut w, &c, tick_at(23 * 60, 0));
        let tick = w.tick();
        let whole = Rect::new(0, 0, c.size.w as i32, c.size.h as i32);
        // Someone standing on a spot takes the player's old tile, then drifts back.
        let (id, spot) = w
            .people()
            .iter()
            .find_map(|p| match p.whereabouts {
                Whereabouts::At {
                    pos,
                    activity: Activity::Working,
                } if p.profession == "sex_worker" => Some((p.id, pos)),
                _ => None,
            })
            .expect("someone on the beat");
        let from = spot
            .neighbors4()
            .find(|n| walk_cost(&c, *n, *n).is_some() && w.person_at(*n).is_none())
            .expect("a free tile beside them");
        assert!(w.swap(id, from, spot));
        w.set_blocker(Some(spot));
        assert_eq!(
            w.person_at(from),
            Some(id),
            "they stand where the player stood"
        );
        assert_eq!(w.person_at(spot), None);
        w.set_blocker(Some(Pos::new(0, 0)));
        w.advance(&c, tick + 1);
        assert_eq!(
            w.person_at(spot),
            Some(id),
            "back on the spot once the player moves on"
        );
        assert_eq!(w.person_at(from), None);
        // A walker steps aside the same way and is back on their path afterwards.
        let on_foot = |w: &World| {
            w.visible(whole).into_iter().find(|m| {
                matches!(m.kind, MoverKind::Walker { id, .. }
                    if matches!(w.people()[id.0 as usize].whereabouts, Whereabouts::Travelling(_)))
            })
        };
        let walker = loop {
            if let Some(m) = on_foot(&w) {
                break m;
            }
            let t = w.tick() + TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
            assert!(
                w.tick() < tick + 120 * TICKS_PER_MINUTE,
                "a walker within two hours"
            );
        };
        let tick = w.tick();
        let MoverKind::Walker { id: wid, .. } = walker.kind else {
            unreachable!()
        };
        let old = walker
            .pos
            .neighbors4()
            .find(|n| walk_cost(&c, *n, *n).is_some() && w.person_at(*n).is_none())
            .expect("a free tile beside the walker");
        assert!(w.swap(wid, old, walker.pos));
        w.set_blocker(Some(walker.pos));
        assert_eq!(w.person_at(old), Some(wid));
        w.set_blocker(None);
        w.advance(&c, tick + 1);
        let back = w
            .visible(whole)
            .into_iter()
            .find(|m| matches!(m.kind, MoverKind::Walker { id, .. } if id == wid))
            .expect("still drawn")
            .pos;
        let Whereabouts::Travelling(trip) = &w.people()[wid.0 as usize].whereabouts else {
            panic!("still walking")
        };
        assert!(
            trip.legs.iter().any(|l| l.path.contains(&back)),
            "back on the path: {back:?}"
        );
        // Nobody inside a building swaps.
        let inside = w
            .people()
            .iter()
            .find(|p| matches!(p.whereabouts, Whereabouts::Inside(_)))
            .unwrap()
            .id;
        assert!(!w.swap(inside, from, spot));
    }

    #[test]
    fn inside_lists_workers_residents_guests_and_visitors() {
        let c = city();
        let mut w = world(&c);
        run_to(&mut w, &c, tick_at(12 * 60, 1));
        let tower = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap()
            .id;
        let at_work = w.inside(tower);
        assert!(!at_work.is_empty(), "a tower has people at noon");
        assert!(at_work.iter().all(|o| o.role == OccupantRole::Working));
        assert!(
            at_work.windows(2).all(|p| p[0].id < p[1].id),
            "sorted by id"
        );
        for o in &at_work {
            let p = &w.people()[o.id.0 as usize];
            assert_eq!(p.profession, "corp_drone");
            assert_eq!(p.whereabouts, Whereabouts::Inside(tower));
        }
        run_to(&mut w, &c, tick_at(3 * 60, 2));
        let block = c
            .buildings
            .iter()
            .find(|b| {
                matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment)
                    && w.inside(b.id)
                        .iter()
                        .any(|o| o.role == OccupantRole::Living)
            })
            .expect("a residential block with people home at three");
        let home = w.inside(block.id);
        for o in home.iter().filter(|o| o.role == OccupantRole::Living) {
            assert_eq!(w.people()[o.id.0 as usize].home, Some(block.id));
        }
        assert!(
            w.inside(tower)
                .iter()
                .all(|o| o.role != OccupantRole::Working),
            "nobody works the tower at three"
        );
        let shop = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        let _ = w.inside(shop.id);
    }

    #[test]
    fn a_two_door_megablock_seeds_and_steps() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let blocks: Vec<_> = c
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Megablock)
            .map(|b| b.id)
            .collect();
        assert!(!blocks.is_empty());
        for b in &blocks {
            let doors = c
                .portals(*b)
                .iter()
                .filter(|(k, _)| *k == StratumKind::Street)
                .count();
            assert!(doors >= 2, "{b:?} has {doors} street doors");
        }
        // The service door the exterior cuts on the north side, (178,57), is one
        // of the megablock's doors for the sim.
        let places = crate::places::Places::derive(&c);
        assert!(
            blocks
                .iter()
                .any(|b| places.doors[b].contains(&Pos::new(178, 57))),
            "the service door is a sim door"
        );
        let mut w = world(&c);
        let mut seen_inside = false;
        for hour in 1..=24u16 {
            let t = tick_at(hour % 24 * 60, if hour == 24 { 2 } else { 1 });
            run_to(&mut w, &c, t);
            seen_inside |= blocks.iter().any(|b| !w.inside(*b).is_empty());
        }
        assert!(seen_inside, "someone goes inside a megablock");
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

    #[test]
    fn work_hours_and_spot_read_the_person() {
        let c = city();
        let w = world(&c);
        let fx = w.people().iter().find(|p| p.profession == "fixer").unwrap();
        assert_eq!(w.work_hours(fx.id), Some((1080, 240)));
        assert!(matches!(fx.work, Work::Spot(_)));
        assert_eq!(
            w.spot_of(fx.id),
            match fx.work {
                Work::Spot(p) => Some(p),
                _ => None,
            }
        );
        let drone = w
            .people()
            .iter()
            .find(|p| p.profession == "corp_drone")
            .unwrap();
        assert_eq!(w.spot_of(drone.id), None);
        assert_eq!(w.work_hours(drone.id), Some(drone.shift));
    }

    #[test]
    fn pos_of_reads_the_street_and_not_the_indoors() {
        let c = city();
        let mut w = world(&c);
        let fx = w
            .people()
            .iter()
            .find(|p| p.profession == "fixer")
            .unwrap()
            .id;
        let spot = w.spot_of(fx).unwrap();
        w.park(fx, spot);
        assert_eq!(w.pos_of(fx), Some(spot));
        let home = w
            .people()
            .iter()
            .find(|p| matches!(p.whereabouts, Whereabouts::Inside(_)))
            .unwrap()
            .id;
        assert_eq!(w.pos_of(home), None);
    }

    #[test]
    fn a_dead_person_is_gone_from_every_lookup() {
        let c = city();
        let mut w = world(&c);
        let fx = w
            .people()
            .iter()
            .find(|p| p.profession == "fixer")
            .unwrap()
            .id;
        let spot = w.spot_of(fx).unwrap();
        w.park(fx, spot);
        assert_eq!(w.person_at(spot), Some(fx));
        w.kill(fx);
        assert!(w.is_dead(fx));
        assert_eq!(w.pos_of(fx), None);
        assert_ne!(w.person_at(spot), Some(fx));
        let rect = Rect::new(spot.x - 3, spot.y - 3, 7, 7);
        assert!(!w.visible(rect).iter().any(|m| matches!(
            m.kind,
            MoverKind::Walker { id, .. } if id == fx
        )));
        for b in &c.buildings {
            assert!(!w.inside(b.id).iter().any(|o| o.id == fx));
        }
        assert_eq!(w.people()[fx.0 as usize].id, fx);
    }

    #[test]
    fn a_dead_person_stays_gone_an_hour_later() {
        let c = city();
        let mut w = world(&c);
        let mut t = tick_at(7 * 60, 1);
        let mut who = None;
        for _ in 0..600 {
            run_to(&mut w, &c, t);
            who = w
                .people()
                .iter()
                .find(|p| matches!(p.whereabouts, Whereabouts::Travelling(_)))
                .map(|p| p.id);
            if who.is_some() {
                break;
            }
            t += TICKS_PER_MINUTE;
        }
        let id = who.expect("someone is travelling");
        w.kill(id);
        run_to(&mut w, &c, t + 60 * TICKS_PER_MINUTE);
        assert_eq!(w.pos_of(id), None);
        for b in &c.buildings {
            assert!(!w.inside(b.id).iter().any(|o| o.id == id));
        }
    }

    #[test]
    fn a_world_comes_back_and_goes_on_the_same() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let people = People::load().unwrap();
        let mut w = World::seed(&c, &people, &SimParams::default(), 22 * 60, 0);
        run_to(&mut w, &c, 3_000);
        let bytes = rmp_serde::to_vec_named(&w).unwrap();
        let mut back: World = rmp_serde::from_slice(&bytes).unwrap();
        back.rebuild(&c, &people);
        run_to(&mut w, &c, 6_000);
        run_to(&mut back, &c, 6_000);
        assert_eq!(w.minute_of_day(), back.minute_of_day());
        assert_eq!(w.people().len(), back.people().len());
        for (a, b) in w.people().iter().zip(back.people()) {
            assert_eq!(a.whereabouts, b.whereabouts, "{}", a.name);
        }
    }

    #[test]
    fn a_world_saved_before_anyone_commuted_takes_it_up() {
        // with no home in the city, as the sim before commuting left them: inside at work
        // with nowhere to go, or at their spot, and none off the map
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let people = People::load().unwrap();
        let mut w = World::seed(&c, &people, &SimParams::default(), 22 * 60, 0);
        let somewhere = *w.places.doors.keys().min_by_key(|b| b.index()).unwrap();
        // (a spot worker with no home keeps to their spot: not among them)
        let without: Vec<PersonId> = w
            .people
            .iter()
            .filter(|p| p.home.is_none() && !matches!(p.work, Work::Spot(_)))
            .map(|p| p.id)
            .collect();
        assert!(without.len() > 100);
        for &id in &without {
            let p = &mut w.people[id.0 as usize];
            p.goal = Goal::Home;
            p.whereabouts = match p.work {
                Work::Building(b) => Whereabouts::Inside(b),
                Work::Spot(pos) => Whereabouts::At {
                    pos,
                    activity: Activity::Working,
                },
                Work::Roaming => Whereabouts::Inside(somewhere),
            };
        }
        let bytes = rmp_serde::to_vec_named(&w).unwrap();
        let mut back: World = rmp_serde::from_slice(&bytes).unwrap();
        back.rebuild(&c, &people);
        let ways = Places::derive(&c).ways_in;
        let (mut went, mut came) = (BTreeSet::new(), BTreeSet::new());
        // to ten the morning after next: the evening out, the morning in
        let end = back.tick() + 36 * 60 * TICKS_PER_MINUTE;
        while back.tick() < end {
            let t = back.tick() + TICKS_PER_MINUTE;
            back.advance(&c, t);
            back.path_some(&c, usize::MAX);
            for &id in &without {
                match &back.people[id.0 as usize].whereabouts {
                    Whereabouts::Away => {
                        went.insert(id);
                    }
                    Whereabouts::Travelling(t) if ways.contains(&t.from) => {
                        came.insert(id);
                    }
                    _ => {}
                }
            }
        }
        // nearly all have gone home out of town, and come back in
        assert!(
            went.len() * 10 >= without.len() * 9,
            "{} of {} went",
            went.len(),
            without.len()
        );
        assert!(
            came.len() * 2 >= without.len(),
            "{} of {} came in",
            came.len(),
            without.len()
        );
    }

    #[test]
    fn rename_renames() {
        let c = city();
        let mut w = world(&c);
        let id = w.people()[3].id;
        w.rename(id, "Mezz");
        assert_eq!(w.people()[id.0 as usize].name, "Mezz");
    }
}
