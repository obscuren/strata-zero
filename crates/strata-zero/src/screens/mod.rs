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
pub mod manual;
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
/// world whose home city doesn't build, or can't play the whole story, gives way to the
/// next in the seed's line, so a new game always starts somewhere its story plays to
/// the end. Only a new game comes through here: a run reopens on the city it saved.
pub fn build_home(params: citygen::GenParams) -> anyhow::Result<citygen::City> {
    let mut scratch = crate::app::AppState::new();
    build_home_where(params, |city| plays_the_story(&mut scratch, city))
}

/// Whether a new game in `city` plays the whole story: a flat for the runner, and from
/// it somebody for every role the missions need (the second fixer, the doctor), cast
/// on `scratch` as the story's start casts them.
fn plays_the_story(scratch: &mut crate::app::AppState, city: &citygen::City) -> bool {
    scratch.enter_city(city.clone());
    crate::missions::uncast_missions(scratch).is_some_and(|uncast| uncast.is_empty())
}

/// `build_home`, taking the first world in the seed's line whose home city `fits`.
fn build_home_where(
    params: citygen::GenParams,
    mut fits: impl FnMut(&citygen::City) -> bool,
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
            Ok(_) => {
                last = Some(anyhow::anyhow!(
                    "seed {seed}'s home city can't play the story"
                ))
            }
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
        // a new game's home city takes its districts from the overworld's colours, and
        // the world colours it with what the story needs: the blocks for the runner's
        // home, a night market for the cook's noodle bar across town (not in your own
        // building), a ripperdoc in a clinic, and the strip's two fixers, Mezz and Coz
        // (the world's colours left 2 and 19 without a market, 11 and 20 without the
        // strip, before the story's needs were given to the home city)
        for seed in [41u64, 3, 8, 26, 2, 11, 19, 20] {
            let mut st = AppState::new();
            let city = super::build_home(st.params.clone().with_seed(seed)).unwrap();
            for need in [
                citygen::Theme::NightMarket,
                citygen::Theme::NeonStrip,
                citygen::Theme::Megablocks,
            ] {
                assert!(
                    city.districts.iter().any(|d| d.theme == need),
                    "seed {seed}: no {need:?} district"
                );
            }
            st.enter_city(city);
            let home = st.home.expect("a home rig").home.building;
            crate::missions::begin(&mut st);
            let uncast: Vec<&str> = st
                .missions
                .runs
                .iter()
                .zip(&st.book.missions)
                .filter(|(r, _)| r.state == crate::missions::RunState::Uncast)
                .map(|(_, m)| m.id.as_str())
                .collect();
            assert!(uncast.is_empty(), "seed {seed}: uncast {uncast:?}");
            let city = st.city.as_ref().unwrap();
            let works_at = |role: &str| {
                let who = st.missions.cast[role];
                crate::missions::cast::work_building(&st, who).map(|b| (b, city.building(b).kind))
            };
            let (cook, kind) = works_at("marlo").expect("the cook works indoors");
            assert_eq!(kind, citygen::BuildingKind::NoodleBar, "seed {seed}");
            assert_ne!(cook, home, "seed {seed}: the cook is in your own building");
            assert_eq!(
                works_at("odile").map(|(_, k)| k),
                Some(citygen::BuildingKind::Clinic),
                "seed {seed}"
            );
            assert_ne!(
                st.missions.cast["mezz"], st.missions.cast["coz"],
                "seed {seed}: two fixers"
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
    fn a_new_game_passes_over_a_home_that_cant_cast_the_story() {
        use crate::missions::{Pick, uncast_missions};
        let params = AppState::new().params;
        // a city with one fixer has a flat but nobody for Coz: the ledger stays uncast
        // and the act never ends, so it is no new game's home
        let one_fixer = citygen::Generator::run(citygen::GenParams {
            seed: 1,
            size: citygen::SizePreset::Medium,
            districts: 6,
            ..Default::default()
        })
        .unwrap();
        let mut scratch = AppState::new();
        scratch.enter_city(one_fixer.clone());
        assert!(scratch.home.is_some(), "a flat");
        assert_eq!(uncast_missions(&mut scratch), Some(vec!["ledger".into()]));
        assert!(!super::plays_the_story(&mut scratch, &one_fixer));
        // seed 7's home plays the story (one ripperdoc, three fixers) ...
        let seven = super::build_home(params.clone().with_seed(7)).unwrap();
        assert_eq!(seven.seed, 7);
        // ... but not with no doctor to cast
        let mut no_doctor = AppState::new();
        no_doctor.book.cast.get_mut("odile").unwrap().pick = Pick::Nearest("astronaut".into());
        no_doctor.enter_city(seven.clone());
        let uncast = uncast_missions(&mut no_doctor);
        assert_eq!(uncast, Some(vec!["doctor".into(), "ledger".into()]));
        assert!(!super::plays_the_story(&mut no_doctor, &seven));
        // every home city has the strip's fixers now, so the line is shown with Coz a
        // second ripperdoc: seed 7's home, with one, gives way to the next in its line
        let mut two_doctors = AppState::new();
        two_doctors.book.cast.get_mut("coz").unwrap().pick = Pick::Worker {
            professions: vec!["ripperdoc".into()],
            at: vec![],
            range: (40, 200),
        };
        let line = params.clone().with_seed(7);
        let fits = |c: &citygen::City| super::plays_the_story(&mut two_doctors, c);
        let city = super::build_home_where(line, fits).unwrap();
        assert_ne!(city.seed, 7);
        two_doctors.enter_city(city);
        assert_eq!(uncast_missions(&mut two_doctors), Some(vec![]));
    }

    #[test]
    fn every_new_games_home_casts_act_one() {
        // seeds spread as a new game's are (up to a billion): each home city casts every
        // role act one's missions need, Coz and the doctor among them, so the act ends
        let params = AppState::new().params;
        for i in 0..16 {
            let seed = citygen::rng::hash64(0xAC7, i) % 1_000_000_007;
            let mut st = AppState::new();
            st.enter_city(super::build_home(params.clone().with_seed(seed)).unwrap());
            crate::missions::begin(&mut st);
            let book = &st.book;
            let needed: std::collections::BTreeSet<String> = book
                .missions
                .iter()
                .flat_map(|m| book.needs_cast(m))
                .collect();
            assert!(
                needed.contains("coz") && needed.contains("odile"),
                "{needed:?}"
            );
            let missing: Vec<&String> = needed
                .iter()
                .filter(|r| !st.missions.cast.contains_key(*r))
                .collect();
            assert!(missing.is_empty(), "seed {seed}: no {missing:?}");
            let uncast: Vec<&str> = (book.missions.iter().zip(&st.missions.runs))
                .filter(|(_, r)| r.state == crate::missions::RunState::Uncast)
                .map(|(m, _)| m.id.as_str())
                .collect();
            assert!(uncast.is_empty(), "seed {seed}: uncast {uncast:?}");
        }
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
