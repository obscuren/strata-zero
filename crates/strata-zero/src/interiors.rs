//! The interiors this session has walked into: plans and floors, generated on the
//! first visit and kept for the session. Never saved; the seed brings them back.

use crate::app::AppState;
use crate::render::map::MapRef;
use citygen::{
    BuildingId, City, Exit, Floor, FloorPlan, Place, Pos, StratumKind, Terrain, generate_floor,
};
use std::collections::HashMap;
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
