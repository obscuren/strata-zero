//! The interiors of the city's closed buildings: a plan made the first time a building
//! is needed and a floor the first time it is visited, both kept. They are saved with the
//! run whole, not remade from the seed, so a building keeps the plan it was first given:
//! a hotel an old save holds keeps today's builder, and one it never saw is laid out as a
//! venue.

use crate::app::AppState;
use crate::render::map::MapRef;
use citygen::rng::hash64;
use citygen::venue::VenueLayout;
use citygen::{
    BuildingId, City, Exit, Floor, FloorPlan, LinkKind, Place, Pos, Role, RoomKind, StratumKind,
    Terrain, generate_floor,
};
use sim::{HouseholdId, Occupant, OccupantRole, PersonId, Whereabouts, World};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Interiors {
    plans: HashMap<BuildingId, FloorPlan>,
    floors: HashMap<(BuildingId, i32), Floor>,
    /// Who lives in which flat, per residential building.
    homes: HashMap<BuildingId, Homes>,
    /// The venues a mission holds, each with its copy: a live one's plan is cut to the
    /// copy's floors, and its floors are drawn with the mission's layer. After the first
    /// saves, so it defaults: they hold none.
    #[serde(default)]
    copies: BTreeMap<BuildingId, crate::venues::MissionCopy>,
    /// Whether each building asked about with no plan yet would be laid out as a venue:
    /// a cache, made from the city when first asked (saving rule 16), never saved.
    #[serde(skip)]
    venues: RefCell<BTreeMap<BuildingId, bool>>,
}

/// Which household lives in which flat of one building: stable, in or out.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
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

/// The plan of closed building `b` in `plans`, made on first ask.
fn plan_in<'a>(
    plans: &'a mut HashMap<BuildingId, FloorPlan>,
    city: &City,
    b: BuildingId,
) -> Option<&'a FloorPlan> {
    match plans.entry(b) {
        Entry::Occupied(e) => Some(e.into_mut()),
        Entry::Vacant(e) => Some(e.insert(FloorPlan::new(city, b)?)),
    }
}

/// Level `level` of `plan` drawn: a live copy's with the mission's layer on it, else
/// from the seed (and a copy whose layer this build can't lay, plain).
fn draw_floor(
    city: &City,
    plan: &FloorPlan,
    copy: Option<&crate::venues::MissionCopy>,
    level: i32,
) -> Floor {
    (copy.filter(|c| c.live))
        .and_then(|c| crate::venues::copy_floor(plan, c, level))
        .unwrap_or_else(|| generate_floor(city, plan, level))
}

/// A venue's flats, (level, number), from its layout.
fn venue_flats(layout: &VenueLayout) -> Vec<(i32, u16)> {
    (layout.levels.iter())
        .flat_map(|x| {
            (x.rooms.iter())
                .filter(|r| r.kind == RoomKind::Flat)
                .map(move |r| (x.level, r.number))
        })
        .collect()
}

impl Interiors {
    /// The plan of a closed building, made on first ask.
    pub fn plan(&mut self, city: &City, b: BuildingId) -> Option<&FloorPlan> {
        plan_in(&mut self.plans, city, b)
    }

    pub fn plan_of(&self, b: BuildingId) -> Option<&FloorPlan> {
        self.plans.get(&b)
    }

    /// Whether building `b`'s plan is a venue's: the session's plan when it holds one (an
    /// old save's hotel kept on the classic builder is none), else whether the seed lays
    /// it out as one, a kind a venue type builds whose layout lays (worked out once).
    pub fn is_venue(&self, city: &City, b: BuildingId) -> bool {
        if let Some(plan) = self.plans.get(&b) {
            return plan.venue.is_some();
        }
        let built = (city.building_opt(b)).is_some_and(|x| {
            citygen::venue::VenuePack::builtin()
                .for_kind(x.kind)
                .is_some()
        });
        built
            && *(self.venues.borrow_mut().entry(b))
                .or_insert_with(|| FloorPlan::new(city, b).is_some_and(|plan| plan.venue.is_some()))
    }

    /// Replaces a building's plan and drops the floors made from the old one.
    pub fn put_plan(&mut self, plan: FloorPlan) {
        self.drop_floors(plan.building);
        self.plans.insert(plan.building, plan);
    }

    /// Drops every floor of building `b`: each is drawn again from its plan when next
    /// needed.
    pub(crate) fn drop_floors(&mut self, b: BuildingId) {
        self.floors.retain(|(x, _), _| *x != b);
    }

    /// The mission's copy of venue `b`, live or waiting.
    pub fn copy(&self, b: BuildingId) -> Option<&crate::venues::MissionCopy> {
        self.copies.get(&b)
    }

    pub(crate) fn copy_mut(&mut self, b: BuildingId) -> Option<&mut crate::venues::MissionCopy> {
        self.copies.get_mut(&b)
    }

    /// Records venue `b`'s copy, or with None lets it go.
    pub(crate) fn set_copy(&mut self, b: BuildingId, copy: Option<crate::venues::MissionCopy>) {
        match copy {
            Some(c) => self.copies.insert(b, c),
            None => self.copies.remove(&b),
        };
    }

    /// Every venue a mission holds, in building order.
    pub(crate) fn copies(&self) -> impl Iterator<Item = (BuildingId, &crate::venues::MissionCopy)> {
        self.copies.iter().map(|(b, c)| (*b, c))
    }

    /// A floor, generated if this is its first visit: a live copy's with the mission's
    /// layer on it. `None` for an open building or a level the plan does not have. An
    /// atrium floor brings the plaza below with it, for the look-down. The plan is
    /// borrowed beside the floors, not copied: a venue's carries its whole layout.
    pub fn ensure(&mut self, city: &City, b: BuildingId, level: i32) -> Option<&Floor> {
        let plan = plan_in(&mut self.plans, city, b)?;
        if !plan.has_level(level) {
            return None;
        }
        if plan.atrium.is_some() && level >= 1 {
            self.floors
                .entry((b, 0))
                .or_insert_with(|| generate_floor(city, plan, 0));
        }
        let copy = self.copies.get(&b);
        Some(
            self.floors
                .entry((b, level))
                .or_insert_with(|| draw_floor(city, plan, copy, level)),
        )
    }

    /// A floor already generated, with its plan.
    pub fn floor(&self, b: BuildingId, level: i32) -> Option<(&FloorPlan, &Floor)> {
        Some((self.plans.get(&b)?, self.floors.get(&(b, level))?))
    }

    /// Works out who lives where in a building, once, counting its flats: on the floors
    /// it generates for them, or a venue's from its layout, drawing no floor. True when
    /// somebody lives there.
    pub fn ensure_homes(&mut self, city: &City, world: &World, b: BuildingId) -> bool {
        if let Some(h) = self.homes.get(&b) {
            return !h.is_empty();
        }
        let Some(plan) = plan_in(&mut self.plans, city, b) else {
            return false;
        };
        let (mut flats, levels) = match plan.venue.as_deref() {
            Some(layout) => (venue_flats(layout), Vec::new()),
            None => (Vec::new(), plan.corridor_floors()),
        };
        for level in levels {
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

/// Walks the player through a closed building's door on the city map, onto the
/// doorstep inside; the floor is generated if this is its first visit. A venue's way in
/// into a deep zone asks its lock from the city side, as a step onto it does inside.
pub fn enter(st: &mut AppState, b: BuildingId, door: (StratumKind, Pos)) -> Result<String, String> {
    if st.net.engaged.is_some() {
        return Err("not now".to_string());
    }
    // A ripperdoc's door is shut while the doc is out or held; its street code lifts it.
    let keyed = if crate::clinic::is_open(st, b) {
        None
    } else {
        let code = crate::credentials::known_street_code(st, b);
        Some(code.ok_or_else(|| crate::clinic::closed_line(st, b))?)
    };
    let city = st.city.as_ref().ok_or("no city")?;
    let plan = st
        .interiors
        .plan(city, b)
        .ok_or("that building has no inside")?;
    let level = plan
        .door_level(door.0)
        .ok_or("that door opens onto nothing")?;
    // A venue's door onto a level it lacks, or a mission's copy doesn't keep, is shut.
    if !plan.has_level(level) {
        return Err("barred".to_string());
    }
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
    let lock = floor.lock_at(exit.at).copied();
    // From the city side: never its free side, which is the doorstep inside.
    let opened = match (lock, st.interiors.plan_of(b)) {
        (Some(lock), Some(plan)) => {
            match crate::venues::meets_lock(st, plan, level, &lock, lock.at) {
                crate::venues::Meets::Held(why) => return Err(why),
                crate::venues::Meets::Through(line) => line,
                crate::venues::Meets::Nothing => None,
            }
        }
        _ => None,
    };
    let name = building_name(city, b);
    crate::credentials::prepare(st, b);
    land(st, Place::Floor { building: b, level }, step);
    Ok(match (keyed, opened) {
        (Some(code), _) => format!("you key in {code:04}"),
        (None, Some(line)) => line,
        (None, None) => format!("Entering `{name}`"),
    })
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
    let onto = onto(city, exit.stratum, landing);
    land(st, Place::Stratum(exit.stratum), landing);
    Ok(format!("Entering {onto}"))
}

/// Where you come out onto at tile `at` of stratum `k`, as a log line names it: the
/// district on the street, the kind of way elsewhere.
fn onto(city: &City, k: StratumKind, at: Pos) -> String {
    match k {
        StratumKind::Street => city.tile(k, at).and_then(|t| t.district).map_or_else(
            || "the `STREET`".to_string(),
            |d| format!("`{}`", city.district(d).name),
        ),
        StratumKind::Skyways => "the `WALKWAY`".to_string(),
        StratumKind::Heights => "the `ROOF`".to_string(),
        StratumKind::Undercity => "the `TUNNEL`".to_string(),
        StratumKind::Net => "the `GRID`".to_string(),
    }
}

/// Crawls in through crawl `way` of building `b` from the city tile you stand on, outside
/// it, onto its grille inside; the floor is generated if this is its first visit. The
/// grille wants a crawler.
pub fn crawl_in(st: &mut AppState, b: BuildingId, way: usize) -> Result<String, String> {
    if st.net.engaged.is_some() {
        return Err("not now".to_string());
    }
    if !crate::venues::can_crawl(st) {
        return Err(crate::venues::screwed_shut());
    }
    let city = st.city.as_ref().ok_or("no city")?;
    let w = (st.interiors.plan(city, b))
        .and_then(|plan| plan.crawls.get(way))
        .copied()
        .ok_or("no crawl here")?;
    let here = st.player.as_ref().map(|p| (p.place, p.pos));
    if here != Some((Place::Stratum(w.stratum), w.outside)) {
        return Err("no crawl here".to_string());
    }
    st.interiors
        .ensure(city, b, w.level)
        .ok_or("that floor is missing")?;
    let name = building_name(city, b);
    crate::credentials::prepare(st, b);
    land(
        st,
        Place::Floor {
            building: b,
            level: w.level,
        },
        w.at,
    );
    Ok(format!("Crawling into `{name}`"))
}

/// Crawls out through the grille under the player, onto the city tile it comes in from.
pub fn crawl_out(st: &mut AppState) -> Result<String, String> {
    let Some((Place::Floor { building, level }, pos)) =
        st.player.as_ref().map(|p| (p.place, p.pos))
    else {
        return Err("no crawl here".to_string());
    };
    let w = (st.interiors.plan_of(building))
        .and_then(|plan| Some(plan.crawls[crate::venues::crawl_way_at(plan, level, pos)?]))
        .ok_or("no crawl here")?;
    if !crate::venues::can_crawl(st) {
        return Err(crate::venues::screwed_shut());
    }
    let onto = onto(st.city.as_ref().ok_or("no city")?, w.stratum, w.outside);
    land(st, Place::Stratum(w.stratum), w.outside);
    Ok(format!("Entering {onto}"))
}

/// The level the player stands on in building `b`, the link under them, and the plan.
fn link_under(st: &mut AppState, b: BuildingId) -> Option<(i32, usize, FloorPlan)> {
    let (level, pos) = match st.player.as_ref().map(|p| (p.place, p.pos))? {
        (Place::Floor { building, level }, pos) if building == b => (level, pos),
        _ => return None,
    };
    let plan = st.interiors.plan(st.city.as_ref()?, b)?.clone();
    let link = plan.link_at(level, pos)?;
    Some((level, link, plan))
}

/// Rides the lift under the player to a floor it stops at: the player steps out at its
/// bank there. A stop that asks for a zone card asks it of a ride there, never of a ride
/// away from it: getting out is free.
pub fn ride(st: &mut AppState, b: BuildingId, level: i32) -> Result<String, String> {
    let (here, link, plan) = link_under(st, b)
        .filter(|(_, link, plan)| plan.links[*link].kind == LinkKind::Lift)
        .ok_or("no lift here")?;
    let at = plan.links[link]
        .stop(level)
        .filter(|_| plan.rides(link, here).contains(&level))
        .ok_or("the lift doesn't stop there")?
        .at;
    let now = st.net.now(st);
    if !st.net.lifts_answer(b, now) {
        return Err("lifts locked down".to_string());
    }
    let card = crate::venues::ride_card(&plan, link, level);
    if crate::credentials::opens_stop(st, b, card).is_none() {
        return Err(format!(
            "the lift wants a {} card",
            crate::venues::zone_name(&plan, card)
        ));
    }
    let city = st.city.as_ref().ok_or("no city")?;
    st.interiors
        .ensure(city, b, level)
        .ok_or("that floor is missing")?;
    crate::credentials::prepare(st, b);
    land(st, Place::Floor { building: b, level }, at);
    Ok(format!("Entering `{}`", FloorPlan::label(level)))
}

/// Takes the stair, escalator, ladder or crawl under the player to its next stop up or
/// down, landing on its landing there. A crawl wants a crawler.
pub fn climb(st: &mut AppState, up: bool) -> Result<String, String> {
    let b = match st.player.as_ref().map(|p| p.place) {
        Some(Place::Floor { building, .. }) => building,
        _ => return Err("no stairs here".to_string()),
    };
    let nothing = || {
        if up {
            "nothing leads up from here".to_string()
        } else {
            "nothing leads down from here".to_string()
        }
    };
    let (level, link, plan) = link_under(st, b)
        .filter(|(_, link, plan)| plan.links[*link].kind != LinkKind::Lift)
        .ok_or_else(nothing)?;
    if plan.links[link].kind == LinkKind::Crawl && !crate::venues::can_crawl(st) {
        return Err(crate::venues::screwed_shut());
    }
    let (next, at) = plan
        .next_stop(link, level, up)
        .and_then(|next| Some((next, plan.links[link].stop(next)?.at)))
        .ok_or_else(nothing)?;
    let city = st.city.as_ref().ok_or("no city")?;
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
        at,
    );
    Ok(format!("Entering `{}`", FloorPlan::label(next)))
}

/// The lift panel's rows for link `link` from `here`: the floors it takes you to and
/// your own, lowest first, marked where they open somewhere.
pub fn floor_rows(plan: &FloorPlan, link: usize, here: i32) -> Vec<(i32, String)> {
    let mut levels = plan.rides(link, here);
    if plan.links.get(link).is_some_and(|l| l.stop(here).is_some()) {
        levels.push(here);
    }
    levels.sort_unstable();
    levels
        .into_iter()
        .map(|level| {
            let mut label = FloorPlan::label(level);
            let mut marks = Vec::new();
            if level == here {
                marks.push("here");
            }
            match plan.door_stratum(level) {
                Some(StratumKind::Skyways) => marks.push("bridge"),
                Some(StratumKind::Heights) => marks.push("roof"),
                _ => {}
            }
            if !marks.is_empty() {
                label = format!("{label} · {}", marks.join(" · "));
            }
            (level, label)
        })
        .collect()
}

/// A building's plan as the session has it, else made fresh from the seed.
pub fn plan_now(st: &AppState, b: BuildingId) -> Option<Cow<'_, FloorPlan>> {
    match st.interiors.plan_of(b) {
        Some(plan) => Some(Cow::Borrowed(plan)),
        None => FloorPlan::new(st.city.as_ref()?, b).map(Cow::Owned),
    }
}

/// A floor as the session has it once walked onto, else made fresh from the seed: the
/// same floor, not kept. So the story can see a floor nobody has looked at.
pub fn floor_now<'a>(st: &'a AppState, plan: &FloorPlan, level: i32) -> Option<Cow<'a, Floor>> {
    if !plan.has_level(level) {
        return None;
    }
    match st.interiors.floor(plan.building, level) {
        Some((_, floor)) => Some(Cow::Borrowed(floor)),
        None => {
            let copy = st.interiors.copy(plan.building);
            Some(Cow::Owned(draw_floor(st.city.as_ref()?, plan, copy, level)))
        }
    }
}

/// Where a person is drawn now (doctor spec §4): their tile on the street, or inside a
/// building the tile its floor's `Placement` stands them on, on the floor they occupy.
/// None for the dead, for anyone behind a door, and for anyone in a building a mission's
/// live copy stands in for: the real building carries on unseen.
pub fn drawn_at(st: &AppState, id: PersonId) -> Option<(Place, Pos)> {
    let world = st.world.as_ref()?;
    if let Some(pos) = world.pos_of(id) {
        return Some((Place::Stratum(StratumKind::Street), pos));
    }
    let Whereabouts::Inside(b) = world.people().get(id.0 as usize)?.whereabouts else {
        return None;
    };
    if crate::venues::copy_live(st, b) {
        return None;
    }
    let occupants = world.inside(b);
    let o = occupants.iter().find(|o| o.id == id)?;
    // Residents and guests are behind their doors: no plan is made for them.
    let plan = match o.role {
        OccupantRole::Living | OccupantRole::Guest => return None,
        OccupantRole::Working | OccupantRole::Visiting => plan_now(st, b)?,
    };
    let level = match o.role {
        OccupantRole::Working if receptionist(world, &occupants) == Some(id) => 0,
        OccupantRole::Working => work_floor(&plan.work_floors(), id),
        _ => 0,
    };
    let floor = floor_now(st, &plan, level)?;
    let none = Homes::default();
    let homes = st.interiors.homes_of(b).unwrap_or(&none);
    let (pos, _) = Placement::new(&plan, &floor, world, homes)
        .people()
        .find(|(_, who)| *who == id)?;
    Some((Place::Floor { building: b, level }, pos))
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

/// The first drone at work in a building: they mind the lobby desk.
fn receptionist(world: &World, occupants: &[Occupant]) -> Option<PersonId> {
    occupants
        .iter()
        .find(|o| {
            o.role == OccupantRole::Working
                && world.people()[o.id.0 as usize].profession == "corp_drone"
        })
        .map(|o| o.id)
}

/// The one of the work floors a worker works on, by who they are.
pub(crate) fn work_floor(work_floors: &[i32], id: PersonId) -> i32 {
    let h = hash64(id.0 as u64, 0x0CC);
    work_floors[(h % work_floors.len() as u64) as usize]
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
        let receptionist = receptionist(world, &occupants);
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
                    if work_floor(&work_floors, o.id) != floor.level {
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
        // Whoever found no station stands on open floor, nearest the first landing.
        let anchor = plan.anchor(floor.level);
        let used: HashSet<Pos> = at.iter().map(|(p, _)| *p).collect();
        let doorsteps: HashSet<Pos> = floor.exits.iter().map(|e| floor.doorstep(e)).collect();
        let mut spots: Vec<Pos> = floor
            .tiles
            .iter()
            .filter(|(p, t)| {
                t.is_walkable()
                    && t.feature.is_none()
                    && !t.is_connector()
                    && t.terrain != Terrain::Door
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
    fn a_doc_is_drawn_at_her_station_whether_her_floor_was_seen_or_not() {
        let (mut st, b, _, _) = crate::clinic::testutil::outside(crate::clinic::testutil::NIGHT);
        let doc = crate::clinic::doc_at_work(&st, b).unwrap();
        // Nobody has walked in: her floor is made fresh for it, and not kept.
        let (place, pos) = drawn_at(&st, doc).expect("drawn inside");
        assert_eq!(st.interiors.generated(), 0);
        let Place::Floor { building, level } = place else {
            panic!("on a floor: {place:?}")
        };
        assert_eq!(building, b);
        let city = st.city.clone().unwrap();
        let floor = st.interiors.ensure(&city, b, level).unwrap();
        assert!(floor.chair().is_some(), "the chair's floor");
        assert_eq!(floor.station_at(pos).map(|s| s.role), Some(Role::Surgery));
        // Seen, the same tile, where the floor's placement stands her.
        assert_eq!(drawn_at(&st, doc), Some((place, pos)));
        let (plan, floor) = st.interiors.floor(b, level).unwrap();
        let placed = Placement::new(plan, floor, st.world.as_ref().unwrap(), &Homes::default());
        assert_eq!(placed.person_at(pos), Some(doc));
        // On the street, where the sim has her.
        let out = crate::missions::tests::outside_door(&st, b);
        st.world.as_mut().unwrap().park(doc, out);
        assert_eq!(
            drawn_at(&st, doc),
            Some((Place::Stratum(StratumKind::Street), out))
        );
        // Dead, nowhere.
        st.world.as_mut().unwrap().kill(doc);
        assert_eq!(drawn_at(&st, doc), None);
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
        let top = i.plan(&c, tower.id).unwrap().top();
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
    fn levels_with_a_gap_climb_and_ride_over_it() {
        let mut st = AppState::new();
        st.enter_city(home_city());
        let city = st.city.clone().unwrap();
        let b = city
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap()
            .id;
        let mut plan = st.interiors.plan(&city, b).unwrap().clone();
        // The links keep their stops on 2F and 3F: they are read through the levels.
        plan.levels.retain(|l| ![2, 3].contains(l));
        st.interiors.put_plan(plan.clone());
        st.interiors.ensure(&city, b, 1).unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        let floor1 = Place::Floor {
            building: b,
            level: 1,
        };
        stand(&mut st, floor1, plan.stair_on(1).unwrap());
        assert_eq!(climb(&mut st, true).unwrap(), "Entering `4F`");
        assert_eq!(climb(&mut st, false).unwrap(), "Entering `1F`");
        stand(&mut st, floor1, plan.lift_on(1).unwrap());
        assert_eq!(
            ride(&mut st, b, 3).unwrap_err(),
            "the lift doesn't stop there"
        );
        assert_eq!(ride(&mut st, b, 4).unwrap(), "Entering `4F`");
        assert!(st.interiors.ensure(&city, b, 2).is_none());
        assert!(
            floor_rows(st.interiors.plan_of(b).unwrap(), 0, 0)
                .iter()
                .all(|(l, _)| ![2, 3].contains(l))
        );
    }

    fn stand(st: &mut AppState, place: Place, pos: Pos) {
        let p = st.player.as_mut().unwrap();
        (p.place, p.pos) = (place, pos);
    }

    /// The home city with the player in it, and its first corp tower with `edit` made
    /// to its plan.
    fn tower_edited(edit: impl FnOnce(&mut FloorPlan)) -> (AppState, BuildingId, FloorPlan) {
        let mut st = AppState::new();
        st.enter_city(home_city());
        let city = st.city.clone().unwrap();
        let b = city
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap()
            .id;
        let mut plan = st.interiors.plan(&city, b).unwrap().clone();
        edit(&mut plan);
        st.interiors.put_plan(plan.clone());
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        (st, b, plan)
    }

    #[test]
    fn an_escalator_and_a_ladder_climb_like_stairs() {
        let stop = |level, at, on, off| citygen::LinkStop {
            level,
            at,
            card: 0,
            on,
            off,
        };
        let (p0, p1, q) = (Pos::new(2, 2), Pos::new(5, 2), Pos::new(2, 5));
        let (mut st, b, _) = tower_edited(|plan| {
            plan.links.push(citygen::LevelLink {
                kind: citygen::LinkKind::Escalator,
                stops: vec![stop(0, p0, true, false), stop(1, p1, false, true)],
                name: None,
            });
            plan.links.push(citygen::LevelLink {
                kind: citygen::LinkKind::Ladder,
                stops: vec![stop(1, q, true, true), stop(2, q, true, true)],
                name: None,
            });
        });
        let city = st.city.clone().unwrap();
        for level in 0..=2 {
            st.interiors.ensure(&city, b, level).unwrap();
        }
        let floor = |level| Place::Floor { building: b, level };
        assert_eq!(
            st.interiors.floor(b, 0).unwrap().1.tiles[p0].terrain,
            Terrain::Escalator
        );
        assert_eq!(
            st.interiors.floor(b, 1).unwrap().1.tiles[q].terrain,
            Terrain::Ladder
        );
        // The map draws each the way it runs from here: the escalator up only.
        let way = |level, p| {
            use crate::render::map::TileMap;
            MapRef::for_place(&city, &st.interiors, &st.constructs, floor(level), 0)
                .unwrap()
                .connector(p)
                .map(|d| (d.kind, d.up, d.down))
        };
        let stair = citygen::ConnectorKind::Stair;
        assert_eq!(way(0, p0), Some((stair, true, false)));
        assert_eq!(way(1, p1), Some((stair, false, false)));
        let ladder = citygen::ConnectorKind::Ladder;
        assert_eq!(way(1, q), Some((ladder, true, false)));
        assert_eq!(way(2, q), Some((ladder, false, true)));
        // Up the escalator, three tiles on; there is no going back down it.
        stand(&mut st, floor(0), p0);
        assert_eq!(climb(&mut st, true).unwrap(), "Entering `1F`");
        let at = |st: &AppState| st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        assert_eq!(at(&st), (floor(1), p1));
        assert_eq!(
            climb(&mut st, false).unwrap_err(),
            "nothing leads down from here"
        );
        // Up the ladder and back down it, on its tile.
        stand(&mut st, floor(1), q);
        assert_eq!(climb(&mut st, true).unwrap(), "Entering `2F`");
        assert_eq!(at(&st), (floor(2), q));
        assert_eq!(climb(&mut st, false).unwrap(), "Entering `1F`");
        assert_eq!(at(&st), (floor(1), q));
    }

    #[test]
    fn the_lift_rides_only_where_it_stops() {
        use crate::screens::popup::{Pick, WorldAct};
        let (mut st, b, plan) = tower_edited(|plan| {
            let top = plan.top();
            assert_eq!(plan.links[0].kind, citygen::LinkKind::Lift);
            plan.links[0]
                .stops
                .retain(|s| [0, 2, top].contains(&s.level));
        });
        let (top, city) = (plan.top(), st.city.clone().unwrap());
        st.interiors.ensure(&city, b, 0).unwrap();
        let lobby = Place::Floor {
            building: b,
            level: 0,
        };
        stand(&mut st, lobby, plan.lift_on(0).unwrap());
        let rows: Vec<(i32, bool)> =
            crate::interact::ride_rows(&st, &mut crate::interact::FREE_LETTERS.chars())
                .unwrap()
                .into_iter()
                .map(|c| match c.pick {
                    Pick::World(WorldAct::Lift(l)) => (l, c.enabled),
                    _ => panic!("a ride row"),
                })
                .collect();
        assert_eq!(rows, vec![(top, true), (2, true), (0, false)]);
        assert_eq!(
            ride(&mut st, b, 1).unwrap_err(),
            "the lift doesn't stop there"
        );
        assert_eq!(ride(&mut st, b, 2).unwrap(), "Entering `2F`");
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos),
            (
                Place::Floor {
                    building: b,
                    level: 2
                },
                plan.lift_on(2).unwrap()
            )
        );
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

    /// Fails today: tables go to visitors in the order they are inside, worked out afresh
    /// at every look, so the visitor before him walking out hands him that table. Placing
    /// the held first moves him the moment he is held instead. Keeping his tile needs the
    /// tile he was held on remembered, which `Placement` (a function of the world alone)
    /// has nowhere to keep.
    #[test]
    #[ignore = "a held visitor's table needs remembering outside Placement"]
    fn a_held_visitor_keeps_his_table_while_others_come_and_go() {
        let c = home_city();
        let people = sim::People::load().unwrap();
        let mut w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        for m in 1..=60 {
            w.advance(&c, m * sim::TICKS_PER_MINUTE);
            w.path_some(&c, usize::MAX);
        }
        // A ground floor with two visitors at its tables: the later one is held.
        let mut i = Interiors::default();
        let at_tables = |plan: &FloorPlan, floor: &Floor, w: &World| -> Vec<(Pos, PersonId)> {
            let visiting: Vec<PersonId> = w
                .inside(plan.building)
                .iter()
                .filter(|o| o.role == OccupantRole::Visiting)
                .map(|o| o.id)
                .collect();
            Placement::new(plan, floor, w, &Homes::default())
                .people()
                .filter(|(pos, id)| {
                    visiting.contains(id)
                        && floor.station_at(*pos).map(|s| s.role) == Some(Role::Table)
                })
                .collect()
        };
        let (b, first, held) = c
            .buildings
            .iter()
            .find_map(|b| {
                i.ensure(&c, b.id, 0)?;
                let (plan, floor) = i.floor(b.id, 0)?;
                let mut seated = at_tables(plan, floor, &w);
                seated.sort_by_key(|(_, id)| id.0);
                (seated.len() >= 2).then(|| (b.id, seated[0].1, seated[1].1))
            })
            .expect("a floor with two visitors at its tables");
        let (plan, floor) = i.floor(b, 0).unwrap();
        let tile = |w: &World, id: PersonId| {
            Placement::new(plan, floor, w, &Homes::default())
                .people()
                .find(|(_, who)| *who == id)
                .map(|(pos, _)| pos)
        };
        let caught = tile(&w, held).expect("drawn at a table");
        w.keep_where(held);
        assert_eq!(tile(&w, held), Some(caught), "held where he sat");
        // The visitor before him walks out: he keeps his table.
        let out = w.doors(b)[0];
        w.park(first, out);
        assert_eq!(tile(&w, held), Some(caught), "the one before him left");
        // The night's comings and goings, on to six: still his.
        let mut churn = 0;
        let mut before: Vec<PersonId> = w
            .inside(b)
            .iter()
            .map(|o| o.id)
            .filter(|id| id.0 < held.0)
            .collect();
        for m in 61..=8 * 60 {
            w.advance(&c, m * sim::TICKS_PER_MINUTE);
            w.path_some(&c, usize::MAX);
            assert_eq!(tile(&w, held), Some(caught), "minute {m}");
            let now: Vec<PersonId> = w
                .inside(b)
                .iter()
                .map(|o| o.id)
                .filter(|id| id.0 < held.0)
                .collect();
            churn += usize::from(now != before);
            before = now;
        }
        assert!(churn > 0, "someone before him came or went");
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
        let top = plan.top();
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

    /// The first hotel of the home city.
    fn first_hotel(city: &City) -> BuildingId {
        city.buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Hotel)
            .expect("a hotel")
            .id
    }

    #[test]
    fn walking_into_a_hotel_finds_a_venue() {
        let mut st = AppState::new();
        st.enter_city(home_city());
        let city = st.city.clone().unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        let hotel = first_hotel(&city);
        let door = city
            .portals(hotel)
            .into_iter()
            .find(|(k, _)| *k == StratumKind::Street)
            .expect("a street door");
        enter(&mut st, hotel, door).unwrap();
        let plan = st.interiors.plan_of(hotel).unwrap();
        assert!(plan.venue.is_some());
        assert!(plan.links.len() >= 2, "{:?}", plan.links);
        let t = citygen::venue::VenuePack::builtin()
            .for_kind(BuildingKind::Hotel)
            .unwrap();
        assert_eq!(plan.lowest(), -(t.levels.below.len() as i32));
        assert_eq!(st.interiors.generated(), 1);
    }

    #[test]
    fn prepare_draws_no_floor_of_a_venue() {
        let mut st = AppState::new();
        st.enter_city(home_city());
        let hotel = first_hotel(st.city.as_ref().unwrap());
        crate::credentials::prepare(&mut st, hotel);
        assert!(st.interiors.plan_of(hotel).unwrap().venue.is_some());
        assert_eq!(st.interiors.generated(), 0);
    }

    #[test]
    fn an_old_saves_hotel_keeps_the_classic_builder() {
        /// A plan as saves wrote it before a building's levels were a list.
        #[derive(serde::Serialize)]
        struct OldPlan {
            building: BuildingId,
            kind: BuildingKind,
            theme: citygen::Theme,
            footprint: citygen::Rect,
            size: citygen::Size,
            lowest: i32,
            top: i32,
            bridge: Option<i32>,
            roof: bool,
            lift: Option<Pos>,
            stair: Pos,
            atrium: Option<citygen::Rect>,
            home_level: Option<i32>,
            clinic: Option<citygen::ClinicPlan>,
        }
        #[derive(serde::Serialize)]
        struct OldInteriors {
            plans: HashMap<BuildingId, OldPlan>,
            floors: HashMap<(BuildingId, i32), Floor>,
            homes: HashMap<BuildingId, Homes>,
        }
        let city = home_city();
        let mut hotels = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Hotel)
            .map(|b| b.id);
        let (hotel, other) = (hotels.next().unwrap(), hotels.next().unwrap());
        let classic = FloorPlan::classic(&city, hotel).unwrap();
        let old = OldPlan {
            building: classic.building,
            kind: classic.kind,
            theme: classic.theme,
            footprint: classic.footprint,
            size: classic.size,
            lowest: classic.lowest(),
            top: classic.top(),
            bridge: classic.door_level(StratumKind::Skyways),
            roof: classic.door_level(StratumKind::Heights).is_some(),
            lift: classic.lift_on(classic.lowest()),
            stair: classic.stair_on(classic.lowest()).unwrap(),
            atrium: classic.atrium,
            home_level: classic.home_level,
            clinic: classic.clinic,
        };
        let saved = OldInteriors {
            plans: HashMap::from([(hotel, old)]),
            floors: HashMap::new(),
            homes: HashMap::new(),
        };
        let bytes = rmp_serde::to_vec_named(&saved).unwrap();
        let mut ints: Interiors = rmp_serde::from_slice(&bytes).unwrap();
        assert!(ints.plan_of(hotel).unwrap().venue.is_none());
        assert_eq!(
            ints.ensure(&city, hotel, 1).unwrap(),
            &generate_floor(&city, &classic, 1)
        );
        // A hotel the save never saw is a venue.
        assert!(ints.plan(&city, other).unwrap().venue.is_some());
    }

    #[test]
    fn workers_stand_at_a_venues_stations() {
        let c = home_city();
        let people = sim::People::load().unwrap();
        let mut w = sim::World::seed(&c, &people, &sim::SimParams::default(), 22 * 60, 0);
        // On to ten in the morning.
        for m in 1..=12 * 60 {
            w.advance(&c, m * sim::TICKS_PER_MINUTE);
            w.path_some(&c, usize::MAX);
        }
        // Nobody works at a hotel: the first office with a drone at work.
        let (office, id) = c
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Office)
            .find_map(|b| Some((b.id, receptionist(&w, &w.inside(b.id))?)))
            .expect("a drone at work in an office at ten");
        let mut i = Interiors::default();
        i.ensure(&c, office, 0).unwrap();
        let (plan, lobby) = i.floor(office, 0).unwrap();
        assert!(plan.venue.is_some());
        let desk = lobby
            .stations
            .iter()
            .find(|s| s.role == Role::Reception)
            .expect("a reception desk")
            .pos;
        let placement = Placement::new(plan, lobby, &w, &Homes::default());
        assert_eq!(placement.person_at(desk), Some(id));
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
