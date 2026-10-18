//! How exposed you are: the light on your tile, and who has you in sight. Stealth
//! and combat read this; today it is a readout.

use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use citygen::lighting::tuning::{DAY_RADIUS, EXPOSURE_BANDS};
use citygen::{Place, Rect, sight};
use sim::{MoverKind, PersonId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exposure {
    /// The light on your tile, ambient included.
    pub light: u8,
    /// Everyone on the stratum who can see you.
    pub eyes: Vec<PersonId>,
}

/// The exposure right now: on a stratum, from the walkers around you; dark and
/// unseen anywhere else.
pub fn of(st: &AppState) -> Exposure {
    let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Exposure::default();
    };
    let Place::Stratum(k) = player.place else {
        return Exposure::default();
    };
    let map = MapRef::stratum_at(city, k, st.minute_of_day());
    let light = map.light(player.pos);
    let Some(world) = st.world.as_ref() else {
        return Exposure {
            light,
            eyes: vec![],
        };
    };
    let radius = player.radius_at(map.daylight());
    let bubble = Rect::new(
        player.pos.x - DAY_RADIUS,
        player.pos.y - DAY_RADIUS,
        2 * DAY_RADIUS + 1,
        2 * DAY_RADIUS + 1,
    );
    let eyes = world
        .visible(bubble)
        .into_iter()
        .filter_map(|m| match m.kind {
            MoverKind::Walker { id, .. } => Some((id, m.pos)),
            _ => None,
        })
        .filter(|(_, from)| {
            sight::sees(
                *from,
                player.pos,
                radius,
                |p| map.tile(p).is_none_or(|t| t.blocks_sight()),
                |p| map.emitted(p),
                |p| map.emits(p),
            )
        })
        .map(|(id, _)| id)
        .collect();
    Exposure { light, eyes }
}

/// The word for a light level.
pub fn band(light: u8) -> &'static str {
    EXPOSURE_BANDS
        .iter()
        .find(|(upto, _)| light < *upto)
        .map_or(EXPOSURE_BANDS[EXPOSURE_BANDS.len() - 1].1, |(_, w)| w)
}

/// The HUD's words: `dim · 2 eyes`, `bright · unseen`.
pub fn text(e: &Exposure) -> String {
    let eyes = match e.eyes.len() {
        0 => "unseen".to_string(),
        1 => "1 eye".to_string(),
        n => format!("{n} eyes"),
    };
    format!("{} · {eyes}", band(e.light))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::{MapRef, TileMap};
    use citygen::{GenParams, Generator, SizePreset};

    fn city_state() -> AppState {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let mut p = crate::walker::Player::spawn(&city, 1);
        p.update_fov(&MapRef::stratum_of(&city, citygen::StratumKind::Street), 0);
        st.world = Some(sim::World::seed(
            &city,
            &st.people,
            &sim::SimParams::default(),
            crate::ui::hud::START_MINUTE_OF_DAY,
            0,
        ));
        st.city = Some(city);
        st.player = Some(p);
        st
    }

    #[test]
    fn bands_and_words() {
        assert_eq!(band(0), "dark");
        assert_eq!(band(39), "dark");
        assert_eq!(band(40), "dim");
        assert_eq!(band(100), "lit");
        assert_eq!(band(180), "bright");
        assert_eq!(band(255), "bright");
        let none = Exposure {
            light: 70,
            eyes: vec![],
        };
        assert_eq!(text(&none), "dim · unseen");
        let two = Exposure {
            light: 200,
            eyes: vec![sim::PersonId(1), sim::PersonId(2)],
        };
        assert_eq!(text(&two), "bright · 2 eyes");
        let one = Exposure {
            light: 10,
            eyes: vec![sim::PersonId(1)],
        };
        assert_eq!(text(&one), "dark · 1 eye");
    }

    #[test]
    fn exposure_reads_the_tile_light_and_only_walkers_who_can_see_you() {
        let mut st = city_state();
        let e = of(&st);
        let p = st.player.as_ref().unwrap();
        let map = MapRef::stratum_at(
            st.city.as_ref().unwrap(),
            citygen::StratumKind::Street,
            st.minute_of_day(),
        );
        assert_eq!(e.light, map.light(p.pos));
        let world = st.world.as_ref().unwrap();
        let bubble = citygen::Rect::new(p.pos.x - 60, p.pos.y - 60, 121, 121);
        for id in &e.eyes {
            let m = world
                .visible(bubble)
                .into_iter()
                .find(|m| matches!(m.kind, sim::MoverKind::Walker { id: w, .. } if w == *id))
                .expect("an eye is a walker on the stratum");
            assert!(m.pos.chebyshev(p.pos) <= 60);
        }
        // Off the strata there is nothing to see you.
        st.player.as_mut().unwrap().place = citygen::Place::Floor {
            building: citygen::BuildingId(0),
            level: 0,
        };
        let inside = of(&st);
        assert_eq!((inside.light, inside.eyes.len()), (0, 0));
    }
}
