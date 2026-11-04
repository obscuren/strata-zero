//! The constructs this session has entered: generated on the first visit and kept
//! for the session, never saved. The gate on the Net leads in; the entry leads out.

use crate::app::AppState;
use crate::render::map::MapRef;
use citygen::{BuildingId, City, Construct, Place, Pos, StratumKind, Terrain, generate_construct};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Constructs {
    made: HashMap<BuildingId, Construct>,
}

impl Constructs {
    /// A building's construct, generated if this is its first visit; `None` for a
    /// building without a subnet.
    pub fn ensure(&mut self, city: &City, b: BuildingId) -> Option<&Construct> {
        match self.made.entry(b) {
            Entry::Occupied(e) => Some(e.into_mut()),
            Entry::Vacant(e) => Some(e.insert(generate_construct(city, b)?)),
        }
    }

    pub fn get(&self, b: BuildingId) -> Option<&Construct> {
        self.made.get(&b)
    }

    pub fn get_mut(&mut self, b: BuildingId) -> Option<&mut Construct> {
        self.made.get_mut(&b)
    }

    /// A construct made elsewhere (the lot's), kept under its building.
    pub fn insert(&mut self, k: Construct) {
        self.made.insert(k.building, k);
    }

    #[cfg(test)]
    pub fn generated(&self) -> usize {
        self.made.len()
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

/// The building's gate tile on the Net stratum.
pub fn city_gate(city: &City, b: BuildingId) -> Option<Pos> {
    let fp = city.building_opt(b)?.footprint;
    fp.border().find(|p| {
        city.tile(StratumKind::Net, *p)
            .is_some_and(|t| t.terrain == Terrain::NetGate && t.building == Some(b))
    })
}

/// Puts the player at a place and tile and shows them around.
pub(crate) fn land(st: &mut AppState, place: Place, pos: Pos) {
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

/// Through a building's gate on the Net, onto the entry inside its construct.
pub fn enter_construct(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    // The first beat falls a full beat after the gate.
    st.net.beat = st.clock_tick();
    let now = st.net.now(st);
    if st.net.alert(b, now) {
        let city = st.city.as_ref().ok_or("no city")?;
        return Err(format!(
            "gate closed · `{}` on alert",
            building_name(city, b)
        ));
    }
    let city = st.city.as_ref().ok_or("no city")?;
    let entry = st
        .constructs
        .ensure(city, b)
        .ok_or("that construct has no inside")?
        .entry;
    let name = building_name(city, b);
    land(st, Place::Construct(b), entry);
    Ok(format!("Entering `{name}`"))
}

/// Out through the entry, onto the gate tile on the Net.
pub fn leave_construct(st: &mut AppState) -> Result<String, String> {
    let city = st.city.as_ref().ok_or("no city")?;
    let player = st.player.as_ref().ok_or("no body")?;
    let Place::Construct(b) = player.place else {
        return Err("not in a construct".to_string());
    };
    let gate = city_gate(city, b).ok_or("the gate is gone")?;
    land(st, Place::Stratum(StratumKind::Net), gate);
    Ok("Entering the `GRID`".to_string())
}
