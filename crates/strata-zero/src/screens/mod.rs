//! Screens: one module per screen, plus shared test helpers.

pub mod atlas;
pub mod bench;
pub mod blueprint;
pub mod broadcast;
pub mod configure;
pub mod console;
pub mod create;
pub mod cyberdeck;
pub mod deck;
pub mod explore;
pub mod files;
pub mod flatlined;
pub mod help;
pub mod jobs_tab;
pub mod keypad;
pub mod kit;
pub mod messages;
pub mod outside;
pub mod pause;
pub mod popup;
pub mod rest;
pub mod runs;
pub mod settings;
pub mod sheet;
pub mod splash;
pub mod stickers;
pub mod terminal;
pub mod title;
pub mod workbench;

use crate::app::{Screen, ScreenCmd, TransitionKind};

/// The screen the app starts on: the splash, which hands over to the title.
pub fn initial(st: &crate::app::AppState) -> Box<dyn Screen> {
    Box::new(splash::Splash::at_launch(st.tick))
}

/// A city built at once from `params`.
pub fn build_city(params: citygen::GenParams) -> anyhow::Result<citygen::City> {
    Ok(citygen::Generator::run(params)?)
}

/// The home city of the world made from `params`' seed, built on its place in that
/// world: its block of the overworld is its map, the world's water and roads in it. A
/// world whose home city doesn't build, or has no flat for the runner, gives way to the
/// next in the seed's line, so a new game always starts somewhere with its story.
pub fn build_home(params: citygen::GenParams) -> anyhow::Result<citygen::City> {
    build_home_where(params, |city| crate::home::HomeRig::find(city).is_some())
}

/// `build_home`, taking the first world in the seed's line whose home city `fits`.
fn build_home_where(
    params: citygen::GenParams,
    fits: impl Fn(&citygen::City) -> bool,
) -> anyhow::Result<citygen::City> {
    const TRIES: u64 = 8;
    let mut last = None;
    for k in 0..TRIES {
        let seed = match k {
            0 => params.seed,
            _ => citygen::rng::hash64(params.seed, k) % 1_000_000_007,
        };
        let world = citygen::overworld::Overworld::generate(crate::outside::world_params(seed))?;
        let home = world.cities.iter().position(|c| c.home).unwrap_or(0) as u8;
        let built = build_city(citygen::sited(&world, home, params.clone().with_seed(seed)));
        match built {
            Ok(city) if fits(&city) => return Ok(city),
            Ok(_) => last = Some(anyhow::anyhow!("seed {seed}'s home city has no flat")),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("no home city built")))
}

/// A new game's city: a fresh seed's home city, on its site.
pub fn new_game_city(st: &crate::app::AppState) -> anyhow::Result<citygen::City> {
    build_home(title::Title::fresh_params(st, st.tick))
}

/// Puts a built city in place and drops onto its street.
pub fn enter_built_city(st: &mut crate::app::AppState, city: citygen::City) -> ScreenCmd {
    st.params = city.params.clone();
    st.enter_city(city);
    ScreenCmd::Switch {
        to: explore_screen(st),
        fx: TransitionKind::Glitch,
    }
}

/// Builds a city from `params` and drops onto its street; a city that fails to build
/// is a warning and nothing more.
pub fn start_city(st: &mut crate::app::AppState, params: citygen::GenParams) -> ScreenCmd {
    start_built(st, build_city(params))
}

/// Drops onto the street of a city just built; one that failed is a warning.
pub fn start_built(
    st: &mut crate::app::AppState,
    built: anyhow::Result<citygen::City>,
) -> ScreenCmd {
    match built {
        Ok(city) => enter_built_city(st, city),
        Err(e) => {
            st.warn(e.to_string());
            ScreenCmd::Stay
        }
    }
}

/// The screen shown once a city exists: the map, on foot, the story begun. Every new
/// game, city load and debug start comes through here; an opened run goes through
/// `resume_screen`.
pub fn explore_screen(st: &mut crate::app::AppState) -> Box<dyn Screen> {
    // a city on its site has its world from the first frame, so the street past its
    // edge draws from the start (a dev start's own city and the lot have none)
    if st
        .city
        .as_ref()
        .is_some_and(|c| c.params.site.is_some() && !c.is_lot())
    {
        crate::outside::ensure(st);
    }
    let map = explore::MapScreen::walking(st);
    crate::missions::begin(st);
    Box::new(map)
}

/// The map over a run opened from its file (CONTINUE, LOAD GAME): you where you stood,
/// the story where it was, nothing started over.
pub fn resume_screen(st: &mut crate::app::AppState) -> Box<dyn Screen> {
    // a run from before the world has it made as it opens, as a new game does
    if st.city.as_ref().is_some_and(|c| !c.is_lot()) {
        crate::outside::ensure(st);
    }
    let outside = st
        .outside
        .as_ref()
        .is_some_and(|o| o.at != crate::outside::At::City);
    if outside {
        Box::new(outside::OutsideScreen::new(st))
    } else {
        Box::new(explore::MapScreen::resumed(st))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{AppState, ScreenCmd};

    #[test]
    fn the_game_opens_on_the_splash() {
        let mut st = AppState::new();
        let mut first = super::initial(&st);
        for tick in 1..super::splash::HAND_OVER {
            st.tick = tick;
            assert!(
                matches!(first.update(&mut st, &[]), ScreenCmd::Stay),
                "{tick}"
            );
        }
        st.tick = super::splash::HAND_OVER;
        assert!(matches!(
            first.update(&mut st, &[]),
            ScreenCmd::Switch { .. }
        ));
    }

    #[test]
    fn a_home_city_casts_the_whole_story() {
        // a home city on its site is smaller than a lone large city: it still has the
        // runner's home and someone for every role the story needs (but a second fixer)
        for seed in [41u64, 3, 8, 26] {
            let mut st = AppState::new();
            let city = super::build_home(st.params.clone().with_seed(seed)).unwrap();
            st.enter_city(city);
            assert!(st.home.is_some(), "seed {seed}: no home rig");
            crate::missions::begin(&mut st);
            let uncast: Vec<&str> = st
                .missions
                .runs
                .iter()
                .zip(&st.book.missions)
                .filter(|(r, _)| r.state == crate::missions::RunState::Uncast)
                .map(|(_, m)| m.id.as_str())
                .collect();
            // a city with one fixer leaves Coz's ledger unplayed, as the story allows
            let fixers = st
                .world
                .as_ref()
                .unwrap()
                .people()
                .iter()
                .filter(|p| p.profession == "fixer")
                .count();
            let allowed: &[&str] = if fixers < 2 { &["ledger"] } else { &[] };
            assert!(
                uncast.iter().all(|m| allowed.contains(m)),
                "seed {seed}: uncast {uncast:?} with {fixers} fixers"
            );
        }
    }

    #[test]
    fn a_new_game_passes_over_a_world_whose_city_has_no_home() {
        // a world whose home city has no flat for the runner (seed 60's did, once) gives
        // way to the next in its seed's line, the same line every time
        let st = AppState::new();
        let refuse = |c: &citygen::City| c.seed != 60;
        let city = super::build_home_where(st.params.clone().with_seed(60), refuse).unwrap();
        assert_ne!(city.seed, 60);
        let again = super::build_home_where(st.params.clone().with_seed(60), refuse).unwrap();
        assert_eq!(again.seed, city.seed, "the same line every time");
        let home = super::build_home(st.params.clone().with_seed(60)).unwrap();
        assert!(crate::home::HomeRig::find(&home).is_some());
    }

    #[test]
    fn a_city_in_the_world_has_it_from_the_first_frame() {
        use crate::walker::Player;
        // a new game's city on its site: the world made before the map's first frame,
        // so the street past its edge draws from the start
        let mut st = AppState::new();
        st.enter_city(crate::outside::tests::sited().clone());
        let _ = super::explore_screen(&mut st);
        assert!(st.outside.is_some(), "a new game's world");
        // a run opened from before the world: made as it opens
        let mut st = AppState::new();
        st.enter_city(crate::runs::testutil::story_city());
        st.player = Some(Player::spawn(st.city.as_ref().unwrap(), 1));
        let _ = super::resume_screen(&mut st);
        assert!(st.outside.is_some(), "an old run's world");
        // the lot and a dev start's own city have none until it's needed
        for city in [citygen::City::lot(1), crate::runs::testutil::story_city()] {
            let mut st = AppState::new();
            st.enter_city(city);
            let _ = super::explore_screen(&mut st);
            assert!(st.outside.is_none());
        }
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        st.player = Some(Player::at(
            citygen::Place::Stratum(citygen::StratumKind::Street),
            citygen::Pos::new(6, 8),
        ));
        let _ = super::resume_screen(&mut st);
        assert!(st.outside.is_none(), "no world past the lot");
    }

    #[test]
    fn new_game_builds_the_home_city_on_its_site() {
        // a new game's params, with a fixed seed for the test
        let st = AppState::new();
        let city = super::build_home(st.params.clone().with_seed(41)).unwrap();
        let site = city
            .params
            .site
            .as_ref()
            .expect("a new game's city has its site");
        assert_eq!(site.world, crate::outside::world_params(city.seed));
        let world = citygen::overworld::Overworld::generate(site.world.clone()).unwrap();
        assert!(
            world.cities[site.city as usize].home,
            "the world's home city"
        );
        let b = world.city_block(site.city);
        assert_eq!(
            (city.size.w, city.size.h),
            ((8 * b.w) as u32, (8 * b.h) as u32),
            "its map is its block"
        );
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::buffer::Buffer;

    pub fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    pub fn buffer_text(b: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..b.area.height {
            for x in 0..b.area.width {
                s.push_str(b[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }
}

#[cfg(test)]
mod everywhere;

#[cfg(test)]
pub(crate) mod shots;
