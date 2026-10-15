//! The interiors this session has walked into: plans and floors, generated on the
//! first visit and kept for the session. Never saved; the seed brings them back.

use crate::app::AppState;
use crate::render::map::MapRef;
use citygen::rng::hash64;
use citygen::{
    BuildingId, City, Exit, Floor, FloorPlan, Place, Pos, Role, RoomKind, StratumKind, Terrain,
    generate_floor,
};
use sim::{OccupantRole, PersonId, World};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub struct Interiors {
    plans: HashMap<BuildingId, FloorPlan>,
    floors: HashMap<(BuildingId, i32), Floor>,
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
    /// a level the plan does not have.
    pub fn ensure(&mut self, city: &City, b: BuildingId, level: i32) -> Option<&Floor> {
        let plan = self.plan(city, b)?.clone();
        if level < plan.lowest || level > plan.top {
            return None;
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

    #[cfg(test)]
    pub fn generated(&self) -> usize {
        self.floors.len()
    }
}

/// What the building is called in a log line.
fn building_name(city: &City, b: BuildingId) -> String {
    city.building_opt(b)
        .map(|x| x.name.clone().unwrap_or_else(|| x.kind.label().to_string()))
        .unwrap_or_else(|| "the building".to_string())
}

/// Puts the player on a floor at a tile and shows them around.
fn land(st: &mut AppState, place: Place, pos: Pos) {
    let bonus = st.character.sight_bonus(&st.content);
    let Some(city) = st.city.as_ref() else { return };
    let Some(player) = st.player.as_mut() else {
        return;
    };
    player.place = place;
    player.pos = pos;
    if let Some(map) = MapRef::for_place(city, &st.interiors, place) {
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
    land(st, Place::Floor { building: b, level }, step);
    Ok(format!("into {name}"))
}

/// Steps out of an exit onto its landing on the city map.
pub fn leave(st: &mut AppState, exit: &Exit) -> String {
    land(st, Place::Stratum(exit.stratum), exit.landing);
    let onto = match exit.stratum {
        StratumKind::Street => "the street",
        StratumKind::Skyways => "the walkway",
        StratumKind::Heights => "the roof",
        StratumKind::Undercity => "the tunnel",
        StratumKind::Net => "the Net",
    };
    format!("out onto {onto}")
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
    st.interiors
        .ensure(city, b, level)
        .ok_or("that floor is missing")?;
    land(st, Place::Floor { building: b, level }, lift);
    Ok(FloorPlan::label(level))
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
    land(
        st,
        Place::Floor {
            building: b,
            level: next,
        },
        pos,
    );
    Ok(FloorPlan::label(next))
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

fn take_station(free: &mut Vec<(Pos, Role)>, wants: &[Role]) -> Option<Pos> {
    for w in wants {
        if let Some(i) = free.iter().position(|(_, r)| r == w) {
            return Some(free.remove(i).0);
        }
    }
    None
}

impl Placement {
    pub fn new(plan: &FloorPlan, floor: &Floor, world: &World) -> Placement {
        let mut free: Vec<(Pos, Role)> = floor.stations.iter().map(|s| (s.pos, s.role)).collect();
        let mut at: Vec<(Pos, PersonId)> = Vec::new();
        let mut owners = HashMap::new();
        let mut extras = Vec::new();
        let work_floors = plan.work_floors();
        let corridor = plan.corridor_floors();
        let flats: Vec<_> = floor
            .rooms
            .iter()
            .filter(|r| r.kind == RoomKind::Flat)
            .collect();
        let hotel_rooms: Vec<_> = floor
            .rooms
            .iter()
            .filter(|r| r.kind == RoomKind::HotelRoom)
            .collect();
        for o in world.inside(plan.building) {
            let p = &world.people()[o.id.0 as usize];
            let h = hash64(o.id.0 as u64, 0x0CC);
            match o.role {
                OccupantRole::Working => {
                    let level = work_floors[(h % work_floors.len() as u64) as usize];
                    if level != floor.level {
                        continue;
                    }
                    let wants: &[Role] = match p.profession.as_str() {
                        "barkeep" => &[Role::Bar, Role::Table],
                        "corp_drone" => &[Role::Reception, Role::Desk],
                        "line_worker" => &[Role::Machine],
                        "ripperdoc" => &[Role::Surgery],
                        "monk" => &[Role::Shrine],
                        _ => &[
                            Role::Desk,
                            Role::Table,
                            Role::Shelf,
                            Role::Bay,
                            Role::Machine,
                            Role::Reception,
                            Role::Bar,
                            Role::Surgery,
                            Role::Shrine,
                        ],
                    };
                    match take_station(&mut free, wants) {
                        Some(pos) => at.push((pos, o.id)),
                        None => extras.push(o.id),
                    }
                }
                OccupantRole::Living | OccupantRole::Guest => {
                    if corridor.is_empty() {
                        continue;
                    }
                    let level = corridor[(h % corridor.len() as u64) as usize];
                    if level != floor.level {
                        continue;
                    }
                    let rooms = if o.role == OccupantRole::Living {
                        &flats
                    } else {
                        &hotel_rooms
                    };
                    if rooms.is_empty() {
                        continue;
                    }
                    let r = rooms[(hash64(h, 1) % rooms.len() as u64) as usize];
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
}
