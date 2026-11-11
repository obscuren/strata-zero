//! A mission's copy of a venue: the building's own floors in their mission state. A
//! mission takes a venue with `begin` and lets it go with `end`; either waits while you're
//! inside, and `tick` carries it out once you're out. Taken, the venue's plan is cut to
//! the copy's floors and its floors are drawn again with the mission's layer on them (laid
//! again from the copy's inputs whenever it's needed), its guards stand at their posts and
//! its cards lie where the layer left them. While it's live every door of the building on
//! the city map is flagged, and a door onto a floor the plan lacks is barred. Ended, the
//! guards and the cards still lying go and the plain building comes back from its layout;
//! what you dropped there stays where it lies, and the codes you learned still work.

use crate::app::AppState;
use citygen::venue::{MissionInputs, MissionLayer, VenuePack, draw_copy_level};
use citygen::{BuildingId, Floor, FloorPlan, Place, Pos, StratumKind};
use game::ItemId;

/// A venue a mission holds, and how far it has got.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MissionCopy {
    pub inputs: MissionInputs,
    /// The levels the copy keeps, lowest first.
    pub kept: Vec<i32>,
    /// Taken: the floors are the copy's. False while it waits for you to be out.
    pub live: bool,
    /// Asked to end: it does once you're out.
    pub ending: bool,
    /// The cards the layer left, where they were left.
    pub left: Vec<(Place, Pos, ItemId)>,
}

/// What a venue's door on the city map shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorMark {
    /// A door into a mission's live copy.
    Flagged,
    /// A door onto a floor the plan lacks: shut.
    Barred,
}

/// The layer of `copy` on `plan`'s layout, laid again from the copy's inputs: None for a
/// plan with no layout, a type this build lacks, or a layer that doesn't lay.
fn layer_of(plan: &FloorPlan, copy: &MissionCopy) -> Option<MissionLayer> {
    let layout = plan.venue.as_deref()?;
    let pack = VenuePack::builtin();
    let t = pack.by_id(&layout.type_id)?;
    MissionLayer::lay(layout, t, pack.size(copy.inputs.size), copy.inputs).ok()
}

/// Level `level` of a copy's floors: the plan's layout drawn with the layer on it.
pub(crate) fn copy_floor(plan: &FloorPlan, copy: &MissionCopy, level: i32) -> Option<Floor> {
    draw_copy_level(plan.venue.as_deref()?, &layer_of(plan, copy)?, level)
}

/// Whether the player stands on a floor of building `b`.
pub(super) fn inside(st: &AppState, b: BuildingId) -> bool {
    matches!(
        st.player.as_ref().map(|p| p.place),
        Some(Place::Floor { building, .. }) if building == b
    )
}

/// Whether building `b`'s floors are a mission's copy now.
pub fn copy_live(st: &AppState, b: BuildingId) -> bool {
    st.interiors.copy(b).is_some_and(|c| c.live)
}

/// Takes a site for a mission. Err when it already holds one, isn't a venue, or can't hold
/// the size.
pub fn begin(st: &mut AppState, b: BuildingId, inputs: MissionInputs) -> Result<String, String> {
    if st.interiors.copy(b).is_some() {
        return Err("a job is already on there".to_string());
    }
    let city = st.city.as_ref().ok_or("no city")?;
    let plan = (st.interiors.plan(city, b)).ok_or("that building has no inside")?;
    let layout = plan.venue.as_deref().ok_or("that building is no venue")?;
    let pack = VenuePack::builtin();
    let t = (pack.by_id(&layout.type_id)).ok_or("that venue's type is gone from this build")?;
    let size = inputs.size.label();
    let layer = MissionLayer::lay(layout, t, pack.size(inputs.size), inputs)
        .map_err(|_| format!("that building can't hold a {size} job"))?;
    let copy = MissionCopy {
        inputs,
        kept: layer.kept,
        live: false,
        ending: false,
        left: Vec::new(),
    };
    st.interiors.set_copy(b, Some(copy));
    tick(st);
    let name = crate::net::building_name(st, b);
    Ok(if copy_live(st, b) {
        format!("a {size} job on `{name}`")
    } else {
        format!("a {size} job on `{name}` · once you're out")
    })
}

/// Lets the site go; it happens at once when you're out, else when you are.
pub fn end(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let copy = (st.interiors.copy_mut(b)).ok_or("no job on there")?;
    copy.ending = true;
    tick(st);
    let name = crate::net::building_name(st, b);
    Ok(if st.interiors.copy(b).is_none() {
        format!("the job on `{name}` is over")
    } else {
        format!("the job on `{name}` ends once you're out")
    })
}

/// Takes a waiting copy, or ends one, once the player is out of its building. A copy whose
/// type this build lacks ends at once (ending needs only the layout, which the plan holds),
/// and the log says it's lost.
pub fn tick(st: &mut AppState) {
    let copies: Vec<(BuildingId, MissionCopy)> = (st.interiors.copies())
        .map(|(b, c)| (b, c.clone()))
        .collect();
    for (b, copy) in copies {
        let typed = (st.interiors.plan_of(b))
            .and_then(|p| p.venue.as_deref())
            .is_some_and(|v| VenuePack::builtin().by_id(&v.type_id).is_some());
        if !typed {
            lose(st, b, &copy);
        } else if inside(st, b) {
            continue;
        } else if copy.ending {
            finish(st, b, &copy);
        } else if !copy.live {
            take(st, b, &copy);
        }
    }
}

/// Takes the copy: the plan cut to its floors, the floors dropped (drawn again with the
/// layer as they're needed), the guards at their posts, the zone cards in the catalogue
/// and those the layer leaves lying where it leaves them.
fn take(st: &mut AppState, b: BuildingId, copy: &MissionCopy) {
    let Some(plan) = st.interiors.plan_of(b) else {
        return;
    };
    let Some(layer) = layer_of(plan, copy) else {
        lose(st, b, copy);
        return;
    };
    let cut = plan.kept(&copy.kept);
    st.interiors.put_plan(cut);
    st.foes.extend(super::guard_foes(&layer, b));
    crate::credentials::stock_zone_cards(st, b);
    let left: Vec<(Place, Pos, ItemId)> = (layer.cards.iter())
        .map(|c| {
            let place = Place::Floor {
                building: b,
                level: c.level,
            };
            (place, c.at, crate::credentials::zone_card_id(b, c.zone))
        })
        .collect();
    for (place, at, id) in &left {
        st.ground.add(*place, *at, id.clone(), 1);
    }
    if let Some(c) = st.interiors.copy_mut(b) {
        c.live = true;
        c.left = left;
    }
    crate::track::rewalk(st);
}

/// Ends the copy: a live one's guards and the cards still lying where it left them go,
/// and the plain building comes back from the layout, its floors drawn again from the
/// seed (the one you stand on at once).
fn finish(st: &mut AppState, b: BuildingId, copy: &MissionCopy) {
    if copy.live {
        st.foes
            .retain(|f| f.post.as_ref().is_none_or(|p| p.building != b));
        for (place, at, id) in &copy.left {
            st.ground.take(*place, *at, id, 1);
        }
        let layout = st.interiors.plan_of(b).and_then(|p| p.venue.clone());
        st.interiors.drop_floors(b);
        if let Some(layout) = layout {
            st.interiors.put_plan(FloorPlan {
                building: b,
                ..FloorPlan::of_layout(*layout)
            });
        }
        let here = st.player.as_ref().map(|p| p.place);
        if let (Some(Place::Floor { building, level }), Some(city)) = (here, st.city.as_ref())
            && building == b
        {
            st.interiors.ensure(city, b, level);
        }
    }
    st.interiors.set_copy(b, None);
    crate::track::rewalk(st);
}

/// Ends a copy this build can't keep, and says it's lost.
fn lose(st: &mut AppState, b: BuildingId, copy: &MissionCopy) {
    let name = crate::net::building_name(st, b);
    finish(st, b, copy);
    st.warn(format!("lost in the update · the job in {name}"));
}

/// The marks on building `b`'s city-map doors, when the session holds it as a venue:
/// barred on a level its plan lacks, flagged on the rest while a copy is live.
fn marks_of(st: &AppState, b: BuildingId) -> Vec<(StratumKind, Pos, DoorMark)> {
    let Some(plan) = st.interiors.plan_of(b) else {
        return Vec::new();
    };
    let Some(v) = plan.venue.as_deref() else {
        return Vec::new();
    };
    let live = copy_live(st, b);
    (v.frame.doors.iter())
        .filter_map(|d| {
            let mark = if !plan.has_level(d.level) {
                DoorMark::Barred
            } else if live {
                DoorMark::Flagged
            } else {
                return None;
            };
            Some((d.stratum, d.door, mark))
        })
        .collect()
}

/// The city-map doors of venue buildings the interiors hold: flagged (a live copy's, on kept
/// levels) and barred (doors on levels the plan lacks).
pub fn door_marks(st: &AppState) -> Vec<(StratumKind, Pos, DoorMark)> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    (city.buildings.iter())
        .flat_map(|x| marks_of(st, x.id))
        .collect()
}

/// The mark on the city-map door on tile `p` of stratum `k`, and its building.
pub fn door_mark_at(st: &AppState, k: StratumKind, p: Pos) -> Option<(BuildingId, DoorMark)> {
    let b = st.city.as_ref()?.tile(k, p)?.building?;
    (marks_of(st, b).into_iter())
        .find(|&(s, q, _)| s == k && q == p)
        .map(|(_, _, mark)| (b, mark))
}

/// The code note on a copy's floor tile: "a sticky note: {code:04} · {door}".
pub fn note_at(st: &AppState, place: Place, pos: Pos) -> Option<String> {
    let Place::Floor { building: b, level } = place else {
        return None;
    };
    let copy = st.interiors.copy(b).filter(|c| c.live)?;
    let layer = layer_of(st.interiors.plan_of(b)?, copy)?;
    let note = (layer.codes.iter()).find(|c| c.level == level && c.at == pos)?;
    let (at, number) = note.lock;
    let code = citygen::rng::door_code(st.city.as_ref()?.seed, b, at, number);
    let door = crate::credentials::door_label(st, b, at, number);
    Some(format!("a sticky note: {code:04} · {door}"))
}

#[cfg(test)]
mod tests;
