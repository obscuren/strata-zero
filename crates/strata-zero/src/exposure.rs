//! How exposed you are: the light on your tile, and who has you in sight. Stealth
//! and combat read this; today it is a readout.

use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use citygen::lighting::tuning::{DAY_RADIUS, EXPOSURE_BANDS};
use citygen::{Place, Rect, StratumKind, sight};
use sim::{MoverKind, PersonId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exposure {
    /// The light on your tile, ambient included.
    pub light: u8,
    /// Everyone on the stratum who can see you.
    pub eyes: Vec<PersonId>,
    /// The street cameras that see you, by their tile.
    pub cameras: Vec<citygen::Pos>,
}

/// The exposure right now: on a stratum, the light where you stand (whoever's cell it
/// is) and the walkers around you; dark and unseen anywhere else.
pub fn of(st: &AppState) -> Exposure {
    let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Exposure::default();
    };
    let Place::Stratum(k) = player.place else {
        return Exposure::default();
    };
    if k == StratumKind::Net {
        return Exposure::default();
    }
    let map = MapRef::stratum_at(city, k, st.minute_of_day());
    // the light on your tile as the map draws it: past the city's edge, and on the cells
    // other areas own, theirs
    let light = match crate::outside::stream::Beyond::of(st) {
        Some(b) => map.beyond(&b).light(player.pos),
        None => map.light(player.pos),
    };
    // The city's walkers are on the Street; above and below it nobody looks.
    let Some(world) = st.world.as_ref().filter(|_| k == StratumKind::Street) else {
        return Exposure {
            light,
            ..Exposure::default()
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
        // Walkers staring at a hijacked ad see nothing else.
        .filter(|(_, from)| !crate::holds::staring(st, *from))
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
    let cameras = crate::cameras::seeing(st).iter().map(|c| c.at).collect();
    Exposure {
        light,
        eyes,
        cameras,
    }
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
    let eyes = match e.eyes.len() + e.cameras.len() {
        0 => "unseen".to_string(),
        1 => "1 eye".to_string(),
        n => format!("{n} eyes"),
    };
    format!("{} · {eyes}", band(e.light))
}

/// The HUD's words on the Net, where nothing is lit or dark: the place, then whether
/// ICE hunts you: `grid · unseen`, `construct · hunted`.
pub fn net_text(place: Place, hunted: bool) -> String {
    let at = match place {
        Place::Construct(_) => "construct",
        _ => "grid",
    };
    let eyes = if hunted { "hunted" } else { "unseen" };
    format!("{at} · {eyes}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_net_reads_the_place_and_the_eyes() {
        use citygen::BuildingId;
        assert_eq!(
            net_text(Place::Stratum(StratumKind::Net), false),
            "grid · unseen"
        );
        assert_eq!(
            net_text(Place::Construct(BuildingId(3)), false),
            "construct · unseen"
        );
        assert_eq!(
            net_text(Place::Construct(BuildingId(3)), true),
            "construct · hunted"
        );
    }
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
            ..Exposure::default()
        };
        assert_eq!(text(&none), "dim · unseen");
        let two = Exposure {
            light: 200,
            eyes: vec![sim::PersonId(1), sim::PersonId(2)],
            ..Exposure::default()
        };
        assert_eq!(text(&two), "bright · 2 eyes");
        let one = Exposure {
            light: 10,
            eyes: vec![sim::PersonId(1)],
            ..Exposure::default()
        };
        assert_eq!(text(&one), "dark · 1 eye");
        let cam = Exposure {
            light: 10,
            eyes: vec![sim::PersonId(1)],
            cameras: vec![citygen::Pos::new(3, 4)],
        };
        assert_eq!(text(&cam), "dark · 2 eyes", "a camera is an eye");
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
        // Walkers live on the Street: above or below it nobody has eyes on you, and
        // the Net has neither light of yours nor eyes.
        let walker = world
            .visible(citygen::Rect::new(0, 0, 1000, 1000))
            .into_iter()
            .find(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
            .expect("someone is out");
        for k in [citygen::StratumKind::Heights, citygen::StratumKind::Skyways] {
            let p = st.player.as_mut().unwrap();
            p.place = citygen::Place::Stratum(k);
            p.pos = walker.pos;
            assert!(of(&st).eyes.is_empty(), "{k:?} has no walkers to see you");
        }
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Stratum(citygen::StratumKind::Net);
        p.pos = walker.pos;
        assert_eq!(of(&st), Exposure::default(), "the Net has no eyes");
        // Off the strata there is nothing to see you.
        st.player.as_mut().unwrap().place = citygen::Place::Floor {
            building: citygen::BuildingId(0),
            level: 0,
        };
        let inside = of(&st);
        assert_eq!((inside.light, inside.eyes.len()), (0, 0));
    }
}
