//! Dev sites: the console's `site` and the title's DEVELOPER rows lay a venue type on any
//! closed building, even one of another kind, make it the building's plan and begin a
//! mission there. A building too low for the size is raised to the type's own top, and a
//! mission that won't lay at the asked seed tries the next few, then the type's other
//! objectives.

use super::begin;
use super::copy::inside;
use crate::app::AppState;
use crate::render::map::MapRef;
use citygen::venue::{
    MissionInputs, MissionLayer, MissionSize, VenueFrame, VenueLayout, VenuePack, VenueType,
    building_seed, lay_out,
};
use citygen::{BuildingId, BuildingKind, FloorPlan, Place, RoomKind, StratumKind, Terrain};
use std::cmp::Reverse;

/// The seeds a mission tries on each objective before it gives up.
const SEEDS: u64 = 8;
/// The buildings a dev start tries, largest first, before it gives up.
const BUILDINGS: usize = 16;

/// The closed building whose city-map door is at or beside `at` on its stratum.
pub fn building_at_door(st: &AppState, at: game::At) -> Option<BuildingId> {
    let Place::Stratum(k) = at.place else {
        return None;
    };
    let city = st.city.as_ref()?;
    std::iter::once(at.pos)
        .chain(at.pos.neighbors4())
        .find_map(|p| {
            let b = (city.tile(k, p))
                .filter(|t| t.terrain == Terrain::Door)?
                .building?;
            let closed = city.building_opt(b)?.kind.is_closed();
            (closed && city.portals(b).contains(&(k, p))).then_some(b)
        })
}

/// Dev: lays type `type_id` on `b` (the building's frame, raised to the type's own top when
/// it can't hold `inputs.size`), makes it the building's plan and begins a mission there.
/// Refuses the home, a building you're inside, an open building and one a job holds.
pub fn dev_site(
    st: &mut AppState,
    b: BuildingId,
    type_id: &str,
    inputs: MissionInputs,
) -> Result<String, String> {
    let (t, inputs) = lay_site(st, b, type_id, inputs, Objectives::Any)?;
    let line = begin(st, b, inputs)?;
    let objective = &t.objectives[inputs.objective].name;
    Ok(format!(
        "{line} · {} · {objective} · seed {}",
        t.name, inputs.seed
    ))
}

/// Dev, from the title's DEVELOPER menu: in a state NEW GAME (DEBUG) has just made (D'SHAWN
/// maxed, a fresh city), picks the largest-footprint closed building away from home, lays
/// `type_id` on it at `size`, begins the mission copy when `mission` (else leaves the plain
/// site), and stands the player on the street outside its main street door. A building
/// with too few doors for the size, or that won't take the type, gives way to the next
/// largest.
pub fn dev_start(
    st: &mut AppState,
    type_id: &str,
    size: MissionSize,
    mission: bool,
) -> Result<BuildingId, String> {
    let inputs = MissionInputs {
        size,
        objective: 0,
        security: 128,
        seed: st.tick,
    };
    dev_start_with(st, type_id, inputs, mission)
}

/// `dev_start` with the mission's inputs given whole: its size, objective, security and
/// seed. A mission that won't lay at those tries the next seeds, then the type's other
/// objectives.
pub fn dev_start_with(
    st: &mut AppState,
    type_id: &str,
    inputs: MissionInputs,
    mission: bool,
) -> Result<BuildingId, String> {
    start_with(st, type_id, inputs, mission, Objectives::Any)
}

/// Dev, the DEVELOPER menu's SITE NET start (Ruling 24 of the plan
/// `docs/superpowers/plans/2026-11-11-site-net.md`): in a state NEW GAME (DEBUG) has
/// just made, the type that builds offices laid on the largest building away from home
/// that holds a MEDIUM job on its first objective in a server room, at `security` and the
/// tick as seed, the mission copy begun, and you on the street at its main door. It keeps
/// to that objective: a building where it won't lay at any of the seeds gives way to the
/// next largest.
pub fn dev_site_net(st: &mut AppState, security: u8) -> Result<BuildingId, String> {
    let t = (VenuePack::builtin().for_kind(BuildingKind::Office))
        .ok_or("no site type builds offices")?;
    let objective = (t.objectives.iter())
        .position(|o| t.room(&o.room).is_some_and(|r| r.kind == RoomKind::Server))
        .ok_or_else(|| format!("no {} job in a server room", t.name))?;
    let inputs = MissionInputs {
        size: MissionSize::Medium,
        objective,
        security,
        seed: st.tick,
    };
    start_with(st, &t.id, inputs, true, Objectives::Asked)
}

/// Which objectives a mission that won't lay at the asked seed may fall back on.
#[derive(Clone, Copy)]
enum Objectives {
    /// The type's others, after the asked one's seeds.
    Any,
    /// The asked one's alone.
    Asked,
}

/// The dev starts: `type_id` laid on the largest building away from home that takes it,
/// its mission begun when `mission`, you on the street at its main door.
fn start_with(
    st: &mut AppState,
    type_id: &str,
    inputs: MissionInputs,
    mission: bool,
    objectives: Objectives,
) -> Result<BuildingId, String> {
    let size = inputs.size;
    let mut why = format!("no building away from home takes a {} job", size.label());
    for b in away_from_home(st, size).into_iter().take(BUILDINGS) {
        let laid = lay_site(st, b, type_id, inputs, objectives).and_then(|(_, inputs)| {
            if mission {
                begin(st, b, inputs).map(drop)
            } else {
                Ok(())
            }
        });
        match laid {
            Ok(()) => {
                stand_outside(st, b)?;
                return Ok(b);
            }
            Err(e) => why = e,
        }
    }
    Err(why)
}

/// The closed buildings but the home with a street door and doors and crawls enough for
/// the ways in `size` asks, largest footprint first.
fn away_from_home(st: &AppState, size: MissionSize) -> Vec<BuildingId> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    let home = st.home.map(|h| h.home.building);
    let ways = usize::from(VenuePack::builtin().size(size).ways_in);
    let mut v: Vec<&citygen::Building> = (city.buildings.iter())
        .filter(|x| x.kind.is_closed() && Some(x.id) != home)
        .filter(|x| {
            VenueFrame::of_building(city, x.id).is_some_and(|f| {
                let open = f.doors.iter().filter(|d| !d.enclosed);
                let street = open.clone().any(|d| d.stratum == StratumKind::Street);
                street && open.count() + f.crawls.len() >= ways
            })
        })
        .collect();
    v.sort_by_key(|x| (Reverse(x.footprint.w * x.footprint.h), x.id));
    v.into_iter().map(|x| x.id).collect()
}

/// Lays type `type_id` on `b` and makes it the building's plan: its frame as the city
/// made it when that holds `inputs.size`, else raised to the type's own top. The inputs a
/// mission lays with come back: `inputs` when they lay, else the next few seeds, then
/// (`Objectives::Any`) the type's other objectives. The building is left as it was when
/// none lays.
fn lay_site(
    st: &mut AppState,
    b: BuildingId,
    type_id: &str,
    inputs: MissionInputs,
    objectives: Objectives,
) -> Result<(&'static VenueType, MissionInputs), String> {
    let pack = VenuePack::builtin();
    let t = (pack.by_id(type_id)).ok_or_else(|| format!("no site type {type_id}"))?;
    let city = st.city.as_ref().ok_or("no city")?;
    let x = city.building_opt(b).ok_or("no such building")?;
    if !x.kind.is_closed() {
        return Err("an open building has no floors".to_string());
    }
    if st.home.is_some_and(|h| h.home.building == b) {
        return Err("not your own building".to_string());
    }
    if inside(st, b) {
        return Err("step outside first".to_string());
    }
    if st.interiors.copy(b).is_some() {
        return Err("a job is already on there".to_string());
    }
    let frame = VenueFrame::of_building(city, b).ok_or("that building has no inside")?;
    let seed = building_seed(city.seed, b);
    let rule = pack.size(inputs.size);
    let own = (lay_out(t, frame.clone(), seed).ok()).filter(|l| l.holds(t, rule));
    let layout = match own {
        Some(l) => l,
        None => lay_out(t, frame.raised(t.frame.top), seed)
            .map_err(|e| format!("a {} won't lay out there: {e}", t.name))?,
    };
    let laid = match objectives {
        Objectives::Any => laid_inputs(&layout, t, inputs),
        Objectives::Asked => laid_seeds(&layout, t, inputs),
    };
    let inputs = laid.ok_or_else(|| {
        format!(
            "that building can't hold a {} {} job",
            inputs.size.label(),
            t.name
        )
    })?;
    st.interiors.put_plan(FloorPlan::of_layout(layout));
    Ok((t, inputs))
}

/// The first inputs a mission lays with on `layout`: `inputs`, then the next seeds, on its
/// objective and then on each other.
fn laid_inputs(
    layout: &VenueLayout,
    t: &VenueType,
    inputs: MissionInputs,
) -> Option<MissionInputs> {
    let others = (0..t.objectives.len()).filter(|&o| o != inputs.objective);
    (std::iter::once(inputs.objective).chain(others)).find_map(|objective| {
        laid_seeds(
            layout,
            t,
            MissionInputs {
                objective,
                ..inputs
            },
        )
    })
}

/// The first inputs a mission lays with on `layout` on its own objective: `inputs`, then
/// the next seeds.
fn laid_seeds(layout: &VenueLayout, t: &VenueType, inputs: MissionInputs) -> Option<MissionInputs> {
    let rule = VenuePack::builtin().size(inputs.size);
    (0..SEEDS)
        .map(|k| MissionInputs {
            seed: inputs.seed.wrapping_add(k),
            ..inputs
        })
        .find(|i| MissionLayer::lay(layout, t, rule, *i).is_ok())
}

/// Stands the player on the street outside building `b`'s main street door (the first
/// that leads somewhere), looking round.
fn stand_outside(st: &mut AppState, b: BuildingId) -> Result<(), String> {
    const STREET: StratumKind = StratumKind::Street;
    let bonus = st.sight_bonus();
    let minute = st.minute_of_day();
    let city = st.city.as_ref().ok_or("no city")?;
    let v = (st.interiors.plan_of(b))
        .and_then(|p| p.venue.as_deref())
        .ok_or("no site there")?;
    let streets = || v.frame.doors.iter().filter(|d| d.stratum == STREET);
    let d = (streets().find(|d| !d.enclosed))
        .or_else(|| streets().next())
        .ok_or("no street door")?;
    let fp = v.frame.footprint;
    let pos = std::iter::once(d.landing)
        .chain(d.door.neighbors4())
        .find(|p| !fp.contains(*p) && city.walkable(STREET, *p))
        .ok_or("nowhere to stand outside")?;
    let place = Place::Stratum(STREET);
    let player = (st.player).get_or_insert_with(|| crate::walker::Player::at(place, pos));
    player.place = place;
    player.pos = pos;
    player.update_fov(&MapRef::stratum_at(city, STREET, minute), bonus);
    st.repaint = true;
    Ok(())
}

#[cfg(test)]
mod tests;
