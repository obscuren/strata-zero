//! The interiors this session has walked into: plans and floors, generated on the
//! first visit and kept for the session. Never saved; the seed brings them back.

use crate::app::AppState;
use crate::render::map::MapRef;
use citygen::rng::hash64;
use citygen::{
    BuildingId, City, Exit, Floor, FloorPlan, Place, Pos, Role, RoomKind, StratumKind, Terrain,
    generate_floor,
};
use sim::{HouseholdId, OccupantRole, PersonId, World};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub struct Interiors {
    plans: HashMap<BuildingId, FloorPlan>,
    floors: HashMap<(BuildingId, i32), Floor>,
    /// Who lives in which flat, per residential building.
    homes: HashMap<BuildingId, Homes>,
}

/// Which household lives in which flat of one building: stable, in or out.
#[derive(Clone, Debug, Default)]
pub struct Homes {
    /// Every flat in the building, lived in or not, in assignment order.
    all: Vec<(i32, u16)>,
    by_household: BTreeMap<HouseholdId, (i32, u16)>,
    /// The households in a flat; the first is the owner.
    residents: BTreeMap<(i32, u16), Vec<HouseholdId>>,
    /// The first member of each household, for naming the owner.
    heads: BTreeMap<HouseholdId, PersonId>,
}

impl Homes {
    /// Household `i` takes flat `i mod flats` of the given order; flats are shared
    /// only once every flat has a household.
    pub(crate) fn new(b: BuildingId, flats: Vec<(i32, u16)>, world: &World) -> Homes {
        let mut homes = Homes {
            all: flats.clone(),
            ..Homes::default()
        };
        if flats.is_empty() {
            return homes;
        }
        let mut households: BTreeMap<HouseholdId, PersonId> = BTreeMap::new();
        for p in world.people().iter().filter(|p| p.home == Some(b)) {
            households.entry(p.household).or_insert(p.id);
        }
        for (i, (h, head)) in households.into_iter().enumerate() {
            let flat = flats[i % flats.len()];
            homes.by_household.insert(h, flat);
            homes.residents.entry(flat).or_default().push(h);
            homes.heads.insert(h, head);
        }
        homes
    }

    /// The flat (level, number) a household lives in.
    pub fn flat_of(&self, household: HouseholdId) -> Option<(i32, u16)> {
        self.by_household.get(&household).copied()
    }

    /// The household that owns a flat.
    pub fn household_of(&self, level: i32, number: u16) -> Option<HouseholdId> {
        self.residents
            .get(&(level, number))
            .and_then(|v| v.first().copied())
    }

    /// The person named at a flat's door.
    pub fn owner_of(&self, level: i32, number: u16) -> Option<PersonId> {
        self.household_of(level, number)
            .and_then(|h| self.heads.get(&h).copied())
    }

    pub fn households(&self) -> impl Iterator<Item = (HouseholdId, (i32, u16))> + '_ {
        self.by_household.iter().map(|(h, f)| (*h, *f))
    }

    /// The person a household's card is named after.
    pub fn head_of(&self, household: HouseholdId) -> Option<PersonId> {
        self.heads.get(&household).copied()
    }

    /// Every flat in the building, lived in or not.
    pub fn all_flats(&self) -> impl Iterator<Item = (i32, u16)> + '_ {
        self.all.iter().copied()
    }

    /// The flats somebody lives in, in level-then-number order.
    pub fn flats(&self) -> impl Iterator<Item = (i32, u16)> + '_ {
        self.residents.keys().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.by_household.is_empty()
    }
}

impl Interiors {
    /// The plan of a closed building, made on first ask.
    pub fn plan(&mut self, city: &City, b: BuildingId) -> Option<&FloorPlan> {
        match self.plans.entry(b) {
            Entry::Occupied(e) => Some(e.into_mut()),
            Entry::Vacant(e) => Some(e.insert(FloorPlan::new(city, b)?)),
        }
    }

    pub fn plan_of(&self, b: BuildingId) -> Option<&FloorPlan> {
        self.plans.get(&b)
    }

    /// A floor, generated if this is its first visit. `None` for an open building or
    /// a level the plan does not have. An atrium floor brings the plaza below with it,
    /// for the look-down.
    pub fn ensure(&mut self, city: &City, b: BuildingId, level: i32) -> Option<&Floor> {
        let plan = self.plan(city, b)?.clone();
        if level < plan.lowest || level > plan.top {
            return None;
        }
        if plan.atrium.is_some() && level >= 1 {
            self.floors
                .entry((b, 0))
                .or_insert_with(|| generate_floor(city, &plan, 0));
        }
        Some(
            self.floors
                .entry((b, level))
                .or_insert_with(|| generate_floor(city, &plan, level)),
        )
    }

    /// A floor already generated, with its plan.
    pub fn floor(&self, b: BuildingId, level: i32) -> Option<(&FloorPlan, &Floor)> {
        Some((self.plans.get(&b)?, self.floors.get(&(b, level))?))
    }

    /// Works out who lives where in a building, once; generates its flat floors to
    /// count the flats. True when somebody lives there.
    pub fn ensure_homes(&mut self, city: &City, world: &World, b: BuildingId) -> bool {
        if let Some(h) = self.homes.get(&b) {
            return !h.is_empty();
        }
        let Some(plan) = self.plan(city, b).cloned() else {
            return false;
        };
        let mut flats = Vec::new();
        for level in plan.corridor_floors() {
            if let Some(floor) = self.ensure(city, b, level) {
                flats.extend(
                    floor
                        .rooms
                        .iter()
                        .filter(|r| r.kind == RoomKind::Flat)
                        .map(|r| (level, r.number)),
                );
            }
        }
        // Round-robin over the levels, so neighbours spread through the building
        // instead of filling it from one floor up.
        flats.sort_unstable();
        let mut by_level: Vec<Vec<(i32, u16)>> = Vec::new();
        for f in flats {
            match by_level.last_mut() {
                Some(l) if l[0].0 == f.0 => l.push(f),
                _ => by_level.push(vec![f]),
            }
        }
        // The seed picks which level the round starts on.
        if !by_level.is_empty() {
            let turn =
                (hash64(city.seed, 0x40E5 + b.index() as u64) % by_level.len() as u64) as usize;
            by_level.rotate_left(turn);
        }
        let most = by_level.iter().map(Vec::len).max().unwrap_or(0);
        let spread: Vec<(i32, u16)> = (0..most)
            .flat_map(|k| by_level.iter().filter_map(move |l| l.get(k).copied()))
            .collect();
        let homes = Homes::new(b, spread, world);
        let lived_in = !homes.is_empty();
        self.homes.insert(b, homes);
        lived_in
    }

    pub fn homes_of(&self, b: BuildingId) -> Option<&Homes> {
        self.homes.get(&b)
    }

    #[cfg(test)]
    pub fn set_homes(&mut self, b: BuildingId, homes: Homes) {
        self.homes.insert(b, homes);
    }

    #[cfg(test)]
    pub fn generated(&self) -> usize {
        self.floors.len()
    }
}

/// What the building is called in a log line.
fn building_name(city: &City, b: BuildingId) -> String {
    city.building_opt(b)
        .map(|x| {
            x.name
                .clone()
                .unwrap_or_else(|| citygen::kind_label(city, b).to_string())
        })
        .unwrap_or_else(|| "the building".to_string())
}

/// Puts the player on a floor at a tile and shows them around.
fn land(st: &mut AppState, place: Place, pos: Pos) {
    let bonus = st.sight_bonus();
    let minute = st.minute_of_day();
    let Some(city) = st.city.as_ref() else { return };
    let Some(player) = st.player.as_mut() else {
        return;
    };
    player.place = place;
    player.pos = pos;
    if let Some(map) = MapRef::for_place(city, &st.interiors, &st.constructs, place, minute) {
        player.update_fov(&map, bonus);
    }
    st.repaint = true;
}

/// The floor a door on a stratum opens onto.
fn level_for(plan: &FloorPlan, stratum: StratumKind) -> Option<i32> {
    match stratum {
        StratumKind::Street => Some(0),
        StratumKind::Skyways => plan.bridge,
        StratumKind::Heights => plan.roof.then_some(plan.top),
        StratumKind::Undercity => (plan.lowest < 0).then_some(-1),
        StratumKind::Net => None,
    }
}

/// Walks the player through a closed building's door on the city map, onto the
/// doorstep inside; the floor is generated if this is its first visit.
pub fn enter(st: &mut AppState, b: BuildingId, door: (StratumKind, Pos)) -> Result<String, String> {
    if st.net.engaged.is_some() {
        return Err("not now".to_string());
    }
    // A ripperdoc's door is shut while the doc is out.
    if !crate::clinic::is_open(st, b) {
        return Err(crate::clinic::closed_line(st, b));
    }
    let city = st.city.as_ref().ok_or("no city")?;
    let plan = st
        .interiors
        .plan(city, b)
        .ok_or("that building has no inside")?
        .clone();
    let level = level_for(&plan, door.0).ok_or("that door opens onto nothing")?;
    let floor = st
        .interiors
        .ensure(city, b, level)
        .ok_or("that floor is missing")?;
    let exit = floor
        .exits
        .iter()
        .find(|e| e.door == door.1)
        .copied()
        .ok_or("no way in here")?;
    let step = floor.doorstep(&exit);
    let name = building_name(city, b);
    crate::credentials::prepare(st, b);
    land(st, Place::Floor { building: b, level }, step);
    Ok(format!("Entering `{name}`"))
}

/// Steps out of an exit onto its landing on the city map.
pub fn leave(st: &mut AppState, exit: &Exit) -> Result<String, String> {
    let city = st.city.as_ref().ok_or("no city")?;
    // The landing, or the nearest free tile beside the door when something stands on it.
    let landing = std::iter::once(exit.landing)
        .chain(exit.door.neighbors4())
        .chain(exit.landing.neighbors8())
        .find(|p| {
            city.walkable(exit.stratum, *p)
                && (*p == exit.landing
                    || city
                        .tile(exit.stratum, *p)
                        .is_some_and(|t| t.terrain != Terrain::Door && !t.is_building_part()))
        })
        .ok_or("the way out is blocked")?;
    // Where you are entering: the district on the street, the kind of way elsewhere.
    let onto = match exit.stratum {
        StratumKind::Street => city
            .tile(exit.stratum, landing)
            .and_then(|t| t.district)
            .map_or_else(
                || "the `STREET`".to_string(),
                |d| format!("`{}`", city.district(d).name),
            ),
        StratumKind::Skyways => "the `WALKWAY`".to_string(),
        StratumKind::Heights => "the `ROOF`".to_string(),
        StratumKind::Undercity => "the `TUNNEL`".to_string(),
        StratumKind::Net => "the `GRID`".to_string(),
    };
    land(st, Place::Stratum(exit.stratum), landing);
    Ok(format!("Entering {onto}"))
}

/// Rides the lift to a floor: the player steps out at that floor's lift bank.
pub fn ride(st: &mut AppState, b: BuildingId, level: i32) -> Result<String, String> {
    let city = st.city.as_ref().ok_or("no city")?;
    let plan = st
        .interiors
        .plan(city, b)
        .ok_or("that building has no inside")?
        .clone();
    let lift = plan.lift.ok_or("no lift here")?;
    if level < plan.lowest || level > plan.top {
        return Err("no such floor".to_string());
    }
    let now = st.net.now(st);
    if !st.net.lifts_answer(b, now) {
        return Err("lifts locked down".to_string());
    }
    st.interiors
        .ensure(city, b, level)
        .ok_or("that floor is missing")?;
    crate::credentials::prepare(st, b);
    land(st, Place::Floor { building: b, level }, lift);
    Ok(format!("Entering `{}`", FloorPlan::label(level)))
}

/// Takes the stairwell one floor up or down, staying on the same tile.
pub fn climb(st: &mut AppState, up: bool) -> Result<String, String> {
    let (b, level, pos) = match st.player.as_ref().map(|p| (p.place, p.pos)) {
        Some((Place::Floor { building, level }, pos)) => (building, level, pos),
        _ => return Err("no stairs here".to_string()),
    };
    let city = st.city.as_ref().ok_or("no city")?;
    let plan = st
        .interiors
        .plan(city, b)
        .ok_or("that building has no inside")?
        .clone();
    let on_stair = st.interiors.floor(b, level).is_some_and(|(_, f)| {
        f.tiles
            .get(pos)
            .is_some_and(|t| t.terrain == Terrain::Stair)
    });
    if !on_stair {
        return Err(if up {
            "nothing leads up from here".to_string()
        } else {
            "nothing leads down from here".to_string()
        });
    }
    let next = if up { level + 1 } else { level - 1 };
    if next < plan.lowest || next > plan.top {
        return Err(if up {
            "nothing leads up from here".to_string()
        } else {
            "nothing leads down from here".to_string()
        });
    }
    st.interiors
        .ensure(city, b, next)
        .ok_or("that floor is missing")?;
    crate::credentials::prepare(st, b);
    land(
        st,
        Place::Floor {
            building: b,
            level: next,
        },
        pos,
    );
    Ok(format!("Entering `{}`", FloorPlan::label(next)))
}

/// The lift panel's rows: every floor, marked where it opens somewhere.
pub fn floor_rows(plan: &FloorPlan, here: i32) -> Vec<(i32, String)> {
    plan.levels()
        .map(|level| {
            let mut label = FloorPlan::label(level);
            let mut marks = Vec::new();
            if level == here {
                marks.push("here");
            }
            if plan.bridge == Some(level) {
                marks.push("bridge");
            }
            if plan.roof && level == plan.top {
                marks.push("roof");
            }
            if !marks.is_empty() {
                label = format!("{label} · {}", marks.join(" · "));
            }
            (level, label)
        })
        .collect()
}

/// Where the building's occupants stand on one floor, worked out from the world's
/// coarse `Inside` state the moment the floor is looked at.
#[derive(Clone, Debug, Default)]
pub struct Placement {
    at: Vec<(Pos, PersonId)>,
    /// A locked room's door and who is behind it.
    owners: HashMap<Pos, PersonId>,
}

/// What a worker with no station of their own takes, in order.
const ANY_STATION: &[Role] = &[
    Role::Desk,
    Role::Table,
    Role::Shelf,
    Role::Bay,
    Role::Machine,
    Role::Reception,
    Role::Bar,
    Role::Surgery,
    Role::Shrine,
];

/// Whether a floor has shops on it: a megablock's plaza.
fn has_shops(floor: &Floor) -> bool {
    floor.rooms.iter().any(|r| r.kind == RoomKind::Shop)
}

fn take_station(free: &mut Vec<(Pos, Role)>, wants: &[Role]) -> Option<Pos> {
    for w in wants {
        if let Some(i) = free.iter().position(|(_, r)| r == w) {
            return Some(free.remove(i).0);
        }
    }
    None
}

impl Placement {
    pub fn new(plan: &FloorPlan, floor: &Floor, world: &World, homes: &Homes) -> Placement {
        let mut free: Vec<(Pos, Role)> = floor.stations.iter().map(|s| (s.pos, s.role)).collect();
        let mut at: Vec<(Pos, PersonId)> = Vec::new();
        let mut owners = HashMap::new();
        // Residents own their flat whether they are in or out.
        for r in floor.rooms.iter().filter(|r| r.kind == RoomKind::Flat) {
            if let (Some(d), Some(owner)) = (r.door, homes.owner_of(floor.level, r.number)) {
                owners.insert(d, owner);
            }
        }
        let mut extras = Vec::new();
        let work_floors = plan.work_floors();
        let corridor = plan.corridor_floors();
        let hotel_rooms: Vec<_> = floor
            .rooms
            .iter()
            .filter(|r| r.kind == RoomKind::HotelRoom)
            .collect();
        let occupants = world.inside(plan.building);
        // The first drone at work minds the lobby desk; the rest spread over the floors.
        let receptionist = occupants
            .iter()
            .find(|o| {
                o.role == OccupantRole::Working
                    && world.people()[o.id.0 as usize].profession == "corp_drone"
            })
            .map(|o| o.id);
        for o in occupants {
            let p = &world.people()[o.id.0 as usize];
            let h = hash64(o.id.0 as u64, 0x0CC);
            match o.role {
                OccupantRole::Working => {
                    if Some(o.id) == receptionist {
                        if floor.level == 0
                            && let Some(pos) = take_station(&mut free, &[Role::Reception])
                        {
                            at.push((pos, o.id));
                        }
                        continue;
                    }
                    let level = work_floors[(h % work_floors.len() as u64) as usize];
                    if level != floor.level {
                        continue;
                    }
                    let shops = has_shops(floor);
                    let wants: &[Role] = match p.profession.as_str() {
                        "barkeep" | "noodle_cook" => &[Role::Bar, Role::Table],
                        // On a megablock plaza, a shop's counter (below), else these.
                        "shopkeeper" if shops => &[Role::Shelf, Role::Desk],
                        "corp_drone" => &[Role::Reception, Role::Desk],
                        "line_worker" => &[Role::Machine],
                        "ripperdoc" => &[Role::Surgery],
                        "monk" => &[Role::Shrine],
                        _ => ANY_STATION,
                    };
                    // A shopkeeper on a megablock plaza minds a shop's counter, not
                    // the lobby desk.
                    let counter = (shops && p.profession == "shopkeeper")
                        .then(|| {
                            free.iter().position(|(pos, r)| {
                                *r == Role::Reception
                                    && floor.rooms.iter().any(|room| {
                                        room.kind == RoomKind::Shop && room.rect.contains(*pos)
                                    })
                            })
                        })
                        .flatten()
                        .map(|i| free.remove(i).0);
                    match counter.or_else(|| take_station(&mut free, wants)) {
                        Some(pos) => at.push((pos, o.id)),
                        None => extras.push(o.id),
                    }
                }
                // Residents are behind their own door, owned above.
                OccupantRole::Living => {}
                OccupantRole::Guest => {
                    if corridor.is_empty() || hotel_rooms.is_empty() {
                        continue;
                    }
                    let level = corridor[(h % corridor.len() as u64) as usize];
                    if level != floor.level {
                        continue;
                    }
                    let r = hotel_rooms[(hash64(h, 1) % hotel_rooms.len() as u64) as usize];
                    if let Some(d) = r.door {
                        owners.entry(d).or_insert(o.id);
                    }
                }
                OccupantRole::Visiting => {
                    if floor.level != 0 {
                        continue;
                    }
                    match take_station(&mut free, &[Role::Table]) {
                        Some(pos) => at.push((pos, o.id)),
                        None => extras.push(o.id),
                    }
                }
            }
        }
        // Whoever found no station stands on open floor, nearest the lift.
        let anchor = plan.lift.unwrap_or(plan.stair);
        let used: HashSet<Pos> = at.iter().map(|(p, _)| *p).collect();
        let doorsteps: HashSet<Pos> = floor.exits.iter().map(|e| floor.doorstep(e)).collect();
        let mut spots: Vec<Pos> = floor
            .tiles
            .iter()
            .filter(|(p, t)| {
                t.is_walkable()
                    && t.feature.is_none()
                    && !matches!(t.terrain, Terrain::Lift | Terrain::Stair | Terrain::Door)
                    && !used.contains(p)
                    && !doorsteps.contains(p)
                    && floor.room_at(*p).is_none_or(|r| !r.private)
            })
            .map(|(p, _)| p)
            .collect();
        spots.sort_by_key(|p| (p.manhattan(anchor), p.y, p.x));
        for (id, p) in extras.into_iter().zip(spots) {
            at.push((p, id));
        }
        Placement { at, owners }
    }

    pub fn person_at(&self, pos: Pos) -> Option<PersonId> {
        self.at.iter().find(|(p, _)| *p == pos).map(|(_, id)| *id)
    }

    /// Who lives or stays behind a locked door.
    pub fn owner_of(&self, door: Pos) -> Option<PersonId> {
        self.owners.get(&door).copied()
    }

    pub fn people(&self) -> impl Iterator<Item = (Pos, PersonId)> + '_ {
        self.at.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingKind, GenParams, Generator, SizePreset};

    /// A Medium city with a megablock and a home in it.
    fn home_city() -> City {
        Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn every_doc_stands_by_the_chair() {
        let mut laundromats = 0;
        for seed in [1u64, 2, 3, 8, 11, 21] {
            let c = Generator::run(GenParams {
                seed,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap();
            let w = World::seed(
                &c,
                &sim::People::load().unwrap(),
                &sim::SimParams::default(),
                22 * 60,
                0,
            );
            for b in c
                .buildings
                .iter()
                .filter(|b| b.kind == BuildingKind::Clinic)
            {
                let plan = FloorPlan::new(&c, b.id).unwrap();
                let front = plan.clinic.unwrap().front;
                laundromats += usize::from(front == citygen::Front::Laundromat);
                let level = plan.work_floors()[0];
                let floor = citygen::generate_floor(&c, &plan, level);
                let spot = floor
                    .stations
                    .iter()
                    .find(|s| s.role == citygen::Role::Surgery)
                    .unwrap()
                    .pos;
                let placed = Placement::new(&plan, &floor, &w, &Homes::default());
                let who = placed.person_at(spot).unwrap_or_else(|| {
                    panic!("seed {seed} {:?} {front:?}: nobody on the doc's spot", b.id)
                });
                assert_eq!(w.people()[who.0 as usize].profession, "ripperdoc");
            }
        }
        assert!(laundromats > 0, "a cellar clinic among them");
    }

    #[test]
    fn a_shut_clinic_turns_you_away() {
        let (mut st, b, door, out) = crate::clinic::testutil::outside(crate::clinic::testutil::DAY);
        let refused = enter(&mut st, b, (StratumKind::Street, door));
        assert_eq!(refused, Err(crate::clinic::closed_line(&st, b)));
        assert!(crate::clinic::closed_line(&st, b).ends_with(" is closed · opens at 18:00"));
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (Place::Stratum(StratumKind::Street), out));
        // At night the doc is in, and the door opens.
        let (mut st, b, door, _) = crate::clinic::testutil::outside(crate::clinic::testutil::NIGHT);
        enter(&mut st, b, (StratumKind::Street, door)).unwrap();
        assert_eq!(
            st.player.as_ref().unwrap().place,
            Place::Floor {
                building: b,
                level: 0
            }
        );
    }

    #[test]
    fn an_atrium_level_brings_the_plaza_with_it() {
        let (city, mut ints) = (home_city(), Interiors::default());
        let h = citygen::home(&city).unwrap();
        ints.ensure(&city, h.building, h.level).unwrap();
        assert!(ints.floor(h.building, 0).is_some());
    }

    #[test]
    fn floors_are_made_once_and_only_inside_the_plan() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap();
        let tower = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap();
        let shop = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        let mut i = Interiors::default();
        assert!(i.plan(&c, shop.id).is_none());
        assert!(i.ensure(&c, shop.id, 0).is_none());
        let top = i.plan(&c, tower.id).unwrap().top;
        assert!(i.ensure(&c, tower.id, 0).is_some());
        assert!(i.ensure(&c, tower.id, 0).is_some());
        assert_eq!(i.generated(), 1);
        assert!(i.ensure(&c, tower.id, top + 1).is_none());
        assert!(i.ensure(&c, tower.id, top).is_some());
        assert_eq!(i.generated(), 2);
        assert!(i.floor(tower.id, 1).is_none());
        assert!(i.floor(tower.id, top).is_some());
    }

    #[test]
    fn placement_leaves_out_the_dead() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap();
        let people = sim::People::load().unwrap();
        let mut w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        for m in 1..=12 * 60 {
            w.advance(&c, m * sim::TICKS_PER_MINUTE);
            w.path_some(&c, usize::MAX);
        }
        let tower = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap();
        let mut i = Interiors::default();
        i.ensure(&c, tower.id, 0).unwrap();
        let (plan, lobby) = i.floor(tower.id, 0).unwrap();
        let who: Vec<PersonId> = Placement::new(plan, lobby, &w, &Homes::default())
            .people()
            .map(|(_, id)| id)
            .collect();
        assert!(!who.is_empty(), "someone stands in the lobby");
        for id in who {
            w.kill(id);
        }
        // Another drone may take the desk; none of the dead stands anywhere.
        let after = Placement::new(plan, lobby, &w, &Homes::default());
        assert!(after.people().all(|(_, id)| !w.is_dead(id)));
    }

    #[test]
    fn the_first_drone_minds_the_lobby_desk() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap();
        let people = sim::People::load().unwrap();
        let mut w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        for m in 1..=12 * 60 {
            w.advance(&c, m * sim::TICKS_PER_MINUTE);
            w.path_some(&c, usize::MAX);
        }
        let tower = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap();
        let mut i = Interiors::default();
        i.ensure(&c, tower.id, 0).unwrap();
        let (plan, lobby) = i.floor(tower.id, 0).unwrap();
        let placement = Placement::new(plan, lobby, &w, &Homes::default());
        let desk = lobby
            .stations
            .iter()
            .find(|s| s.role == citygen::Role::Reception)
            .unwrap()
            .pos;
        let id = placement
            .person_at(desk)
            .expect("someone minds the desk at ten");
        assert_eq!(w.people()[id.0 as usize].profession, "corp_drone");
        // No shops in a tower: a shopkeeper there would take any station as before.
        assert!(!has_shops(lobby));
        // And that drone sits at no desk upstairs.
        let top = plan.top;
        i.ensure(&c, tower.id, top).unwrap();
        let (plan, floor) = i.floor(tower.id, top).unwrap();
        let upstairs = Placement::new(plan, floor, &w, &Homes::default());
        assert!(upstairs.people().all(|(_, who)| who != id));
    }

    #[test]
    fn nobody_else_lives_in_the_players_flat() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let home = citygen::home(&c).expect("a home");
        let people = sim::People::load().unwrap();
        let w = sim::World::seed(&c, &people, &sim::SimParams::default(), 2 * 60, 0);
        let mut i = Interiors::default();
        i.ensure(&c, home.building, home.level).unwrap();
        i.ensure_homes(&c, &w, home.building);
        let homes = i.homes_of(home.building).unwrap().clone();
        let (plan, floor) = i.floor(home.building, home.level).unwrap();
        let placement = Placement::new(plan, floor, &w, &homes);
        let owned = floor
            .rooms
            .iter()
            .filter_map(|r| r.door)
            .filter(|d| placement.owner_of(*d).is_some())
            .count();
        assert!(owned > 0, "the neighbours are home at two");
        assert_eq!(placement.owner_of(home.door), None);
    }

    #[test]
    fn shopkeepers_mind_the_plaza_shops_by_day() {
        let c = home_city();
        let home = citygen::home(&c).expect("a home");
        let people = sim::People::load().unwrap();
        let mut w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        // On to half past twelve.
        for m in 1..=(14 * 60 + 30) {
            w.advance(&c, m * sim::TICKS_PER_MINUTE);
            w.path_some(&c, usize::MAX);
        }
        let mut i = Interiors::default();
        i.ensure(&c, home.building, 0).unwrap();
        let (plan, plaza) = i.floor(home.building, 0).unwrap();
        let placement = Placement::new(plan, plaza, &w, &Homes::default());
        let shop = |p: Pos| {
            plaza
                .rooms
                .iter()
                .find(|r| r.kind != RoomKind::Lobby && r.rect.contains(p))
                .map(|r| r.kind)
        };
        let at_stations: Vec<_> = plaza
            .stations
            .iter()
            .filter_map(|s| placement.person_at(s.pos).map(|id| (s.role, s.pos, id)))
            .collect();
        assert!(!at_stations.is_empty(), "nobody on the plaza at 12:30");
        let seen: Vec<_> = at_stations
            .iter()
            .map(|(r, p, id)| (r, shop(*p), &w.people()[id.0 as usize].profession))
            .collect();
        // The shopkeepers stand behind their shops' counters, not at the lobby desk
        // or the plaza tables.
        let shopkeeper = |id: &sim::PersonId| w.people()[id.0 as usize].profession == "shopkeeper";
        assert!(
            at_stations.iter().any(|(_, _, id)| shopkeeper(id)),
            "{seen:?}"
        );
        for (role, pos, id) in &at_stations {
            if shopkeeper(id) {
                assert_eq!(*role, citygen::Role::Reception, "{seen:?}");
                assert_eq!(shop(*pos), Some(RoomKind::Shop), "{seen:?}");
            }
        }
    }

    fn city6() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn every_resident_has_a_stable_flat_and_flats_are_not_shared_while_free() {
        let c = city6();
        let people = sim::People::load().unwrap();
        let w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        let b = c
            .buildings
            .iter()
            .find(|b| {
                matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
                    && w.people().iter().any(|p| p.home == Some(b.id))
            })
            .expect("a residential building with residents")
            .id;
        let mut a = Interiors::default();
        assert!(a.ensure_homes(&c, &w, b));
        let mut other = Interiors::default();
        assert!(other.ensure_homes(&c, &w, b));
        let (ha, hc) = (a.homes_of(b).unwrap(), other.homes_of(b).unwrap());
        let mut seen: std::collections::BTreeMap<(i32, u16), std::collections::BTreeSet<_>> =
            std::collections::BTreeMap::new();
        for p in w.people().iter().filter(|p| p.home == Some(b)) {
            let flat = ha.flat_of(p.household).expect("a flat");
            assert_eq!(Some(flat), hc.flat_of(p.household), "stable");
            seen.entry(flat).or_default().insert(p.household);
        }
        let households = ha.households().count();
        let flats = ha.flats().count();
        assert!(flats > 0);
        if households <= flats {
            assert!(
                seen.values().all(|hs| hs.len() == 1),
                "no sharing while flats remain: {seen:?}"
            );
        }
        let (level, number) = ha.flats().next().unwrap();
        let h = ha
            .household_of(level, number)
            .expect("the first flat is lived in");
        assert_eq!(
            ha.owner_of(level, number)
                .map(|p| w.people()[p.0 as usize].household),
            Some(h)
        );
    }

    #[test]
    fn a_building_without_flats_has_empty_homes() {
        let c = city6();
        let people = sim::People::load().unwrap();
        let w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        let b = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Office)
            .unwrap()
            .id;
        let mut i = Interiors::default();
        assert!(!i.ensure_homes(&c, &w, b));
        assert!(i.homes_of(b).is_none_or(|h| h.is_empty()));
    }

    #[test]
    fn stepping_out_says_where_you_are_entering() {
        let mut st = AppState::new();
        st.enter_city(home_city());
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        let b = st.home.unwrap().home.building;
        let door = {
            let city = st.city.as_ref().unwrap();
            city.portals(b)
                .into_iter()
                .find(|(k, _)| *k == StratumKind::Street)
                .expect("a street door")
        };
        enter(&mut st, b, door).unwrap();
        let exit = st
            .interiors
            .floor(b, 0)
            .unwrap()
            .1
            .exits
            .iter()
            .find(|e| e.door == door.1)
            .copied()
            .unwrap();
        let line = leave(&mut st, &exit).unwrap();
        assert!(line.starts_with("Entering `"), "{line}");
        assert!(!line.contains("Leaving"), "{line}");
        assert!(!line.contains("the building"), "{line}");
    }
}
