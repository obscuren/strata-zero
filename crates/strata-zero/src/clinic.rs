//! Ripperdoc clinics: a clinic is open while its doc is in, its chair is where chrome
//! goes in and out, and `operating` is the one question everything that changes chrome
//! asks.

use crate::app::AppState;
use citygen::{BuildingId, Place};
use sim::{OccupantRole, PersonId};

/// What a chrome change is refused with anywhere but a ripperdoc's chair.
pub const OUTSIDE: &str = "chrome goes in and out in a ripperdoc's chair";

/// The ripperdoc profession's id.
const RIPPERDOC: &str = "ripperdoc";

/// The chair you are sitting in: the floor it stands on and the doc at it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chair {
    pub building: BuildingId,
    pub level: i32,
    pub doc: PersonId,
}

/// Whether chrome can change, and on whose terms.
#[derive(Clone, Debug, PartialEq)]
pub enum Operating {
    /// In a doc's chair: chrome changes, for the doc's fee.
    Chair { doc: String },
    /// Dev free chrome: anywhere, free.
    Free,
    /// Anywhere else: chrome only looks.
    Locked,
    /// The chair's doc has gone, or you are no longer on its floor.
    Gone { doc: String },
}

/// Whether a building is a ripperdoc's clinic.
pub fn is_clinic(st: &AppState, b: BuildingId) -> bool {
    st.city
        .as_ref()
        .is_some_and(|c| citygen::clinic_front(c, b).is_some())
}

/// The clinic's doc, while they are inside at work.
pub fn doc_at_work(st: &AppState, b: BuildingId) -> Option<PersonId> {
    let world = st.world.as_ref()?;
    world
        .inside(b)
        .into_iter()
        .filter(|o| o.role == OccupantRole::Working)
        .map(|o| o.id)
        .find(|id| world.people()[id.0 as usize].profession == RIPPERDOC)
}

/// A clinic is open while its doc is in; every other building is as it always was.
pub fn is_open(st: &AppState, b: BuildingId) -> bool {
    !is_clinic(st, b) || doc_at_work(st, b).is_some()
}

/// What the clinic calls its doc: "Doc" and their first name.
pub fn doc_name(st: &AppState, id: PersonId) -> String {
    let name = st
        .world
        .as_ref()
        .and_then(|w| w.people().get(id.0 as usize))
        .map_or("", |p| p.name.as_str());
    format!("Doc {}", name.split_whitespace().next().unwrap_or("Nobody"))
}

/// The doc's hours: the profession's day.
pub fn hours(st: &AppState) -> (u16, u16) {
    match st.people.profession(RIPPERDOC).map(|p| &p.hours) {
        Some(sim::Hours::Day { open, close }) => (*open, *close),
        _ => (1080, 240),
    }
}

/// A minute of the day as `18:00`.
pub fn hhmm(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60 % 24, minute % 60)
}

/// The shut door's window: `CLOSED · opens 18:00 · closes 04:00`.
pub fn hours_line(st: &AppState) -> String {
    let (open, close) = hours(st);
    format!("CLOSED · opens {} · closes {}", hhmm(open), hhmm(close))
}

/// What the building is called to the player.
pub fn name(st: &AppState, b: BuildingId) -> String {
    st.city
        .as_ref()
        .and_then(|c| {
            let x = c.building_opt(b)?;
            Some(
                x.name
                    .clone()
                    .unwrap_or_else(|| crate::ui::hud::capitalised(citygen::kind_label(c, b))),
            )
        })
        .unwrap_or_else(|| "The clinic".to_string())
}

/// Walking into a shut clinic: `Hot Pot Lou is closed · opens at 18:00`.
pub fn closed_line(st: &AppState, b: BuildingId) -> String {
    format!("{} is closed · opens at {}", name(st, b), hhmm(hours(st).0))
}

/// The chair's doc has left: `Doc Lou has gone for the night`.
pub fn gone_line(doc: &str) -> String {
    format!("{doc} has gone for the night")
}

/// Whether chrome can change now: free with dev free chrome; in a chair while you are
/// on its floor and its doc is still in; else locked.
pub fn operating(st: &AppState) -> Operating {
    if st.free_chrome {
        return Operating::Free;
    }
    let Some(chair) = st.chair else {
        return Operating::Locked;
    };
    let doc = doc_name(st, chair.doc);
    let here = st.player.as_ref().is_some_and(|p| {
        p.place
            == Place::Floor {
                building: chair.building,
                level: chair.level,
            }
    });
    if here && doc_at_work(st, chair.building) == Some(chair.doc) {
        Operating::Chair { doc }
    } else {
        Operating::Gone { doc }
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use crate::render::map::MapRef;
    use crate::walker::Player;
    use citygen::{City, GenParams, Generator, Pos, SizePreset, StratumKind};
    use std::sync::OnceLock;

    /// Seed 8, medium, seven districts: the shots' city, generated once.
    fn city() -> City {
        static CITY: OnceLock<City> = OnceLock::new();
        CITY.get_or_init(|| {
            Generator::run(GenParams {
                seed: 8,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap()
        })
        .clone()
    }

    /// The city at `minute` of the day, the player on the pavement outside a clinic's
    /// street door: the state, the clinic, its door, and the tile outside it.
    pub fn outside(minute: u16) -> (AppState, BuildingId, Pos, Pos) {
        let mut st = AppState::new();
        st.enter_city_at(city(), minute);
        let c = st.city.as_ref().unwrap();
        let (b, door, out) = c
            .buildings
            .iter()
            .filter(|b| citygen::clinic_front(c, b.id).is_some())
            .find_map(|b| {
                let door = c
                    .portals(b.id)
                    .into_iter()
                    .find(|(k, _)| *k == StratumKind::Street)?
                    .1;
                let out = door
                    .neighbors4()
                    .find(|n| !b.footprint.contains(*n) && c.walkable(StratumKind::Street, *n))?;
                Some((b.id, door, out))
            })
            .expect("a clinic with a street door");
        let place = Place::Stratum(StratumKind::Street);
        let mut p = Player::at(place, out);
        p.update_fov(&MapRef::stratum_at(c, StratumKind::Street, minute), 0);
        st.player = Some(p);
        (st, b, door, out)
    }

    /// The doc keeps night hours: in at this minute, out at `DAY`.
    pub const NIGHT: u16 = 22 * 60;
    pub const DAY: u16 = 12 * 60;

    /// At night, inside the clinic on the chair's floor, standing beside the chair (not
    /// on the doc's spot): the state, the clinic, the chair's level and tile.
    pub fn inside() -> (AppState, BuildingId, i32, Pos) {
        let (mut st, b, _, _) = outside(NIGHT);
        assert!(doc_at_work(&st, b).is_some(), "the doc is in at night");
        let level = if citygen::clinic_front(st.city.as_ref().unwrap(), b)
            == Some(citygen::Front::Laundromat)
        {
            -1
        } else {
            0
        };
        let city = st.city.clone().unwrap();
        let floor = st.interiors.ensure(&city, b, level).unwrap().clone();
        let chair = floor.chair().unwrap();
        let doc_spot = floor
            .stations
            .iter()
            .find(|s| s.role == citygen::Role::Surgery)
            .unwrap()
            .pos;
        let stand = chair
            .neighbors4()
            .find(|n| *n != doc_spot && floor.tiles[*n].is_walkable())
            .unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = Place::Floor { building: b, level };
        p.pos = stand;
        (st, b, level, chair)
    }

    /// `inside`, sat in the chair.
    pub fn seated() -> AppState {
        let (mut st, b, level, _) = inside();
        let doc = doc_at_work(&st, b).unwrap();
        st.chair = Some(Chair {
            building: b,
            level,
            doc,
        });
        st
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::{DAY, NIGHT, inside, outside, seated};
    use super::*;

    #[test]
    fn a_clinic_is_open_while_its_doc_is_in() {
        let (st, b, _, _) = outside(NIGHT);
        assert!(doc_at_work(&st, b).is_some());
        assert!(is_open(&st, b));
        let (st, b, _, _) = outside(DAY);
        assert!(doc_at_work(&st, b).is_none());
        assert!(!is_open(&st, b));
        // Every other building is as it always was.
        let c = st.city.as_ref().unwrap();
        let other = c
            .buildings
            .iter()
            .find(|x| x.kind == citygen::BuildingKind::Bar)
            .unwrap()
            .id;
        assert!(is_open(&st, other));
    }

    #[test]
    fn operating_follows_the_chair() {
        let (mut st, _, _, _) = inside();
        assert_eq!(operating(&st), Operating::Locked);
        let mut sat = seated();
        let Operating::Chair { doc } = operating(&sat) else {
            panic!("{:?}", operating(&sat))
        };
        assert!(doc.starts_with("Doc "), "{doc}");
        // Off the chair's floor, the chair is over.
        sat.player.as_mut().unwrap().place = Place::Stratum(citygen::StratumKind::Street);
        assert_eq!(operating(&sat), Operating::Gone { doc });
        st.free_chrome = true;
        assert_eq!(operating(&st), Operating::Free);
        sat.free_chrome = true;
        assert_eq!(operating(&sat), Operating::Free);
    }

    #[test]
    fn lines_read_as_the_spec() {
        let (mut st, b, _, _) = outside(DAY);
        assert_eq!(hours_line(&st), "CLOSED · opens 18:00 · closes 04:00");
        st.city.as_mut().unwrap().buildings[b.index()].name = Some("Hot Pot Lou".to_string());
        assert_eq!(
            closed_line(&st, b),
            "Hot Pot Lou is closed · opens at 18:00"
        );
        assert_eq!(gone_line("Doc Lou"), "Doc Lou has gone for the night");
        assert_eq!(hhmm(1080), "18:00");
        assert_eq!(hhmm(240), "04:00");
    }
}
