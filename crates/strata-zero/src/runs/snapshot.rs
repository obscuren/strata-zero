//! The run as one value: every field of `AppState` that is the world or your progress.
//!
//! `snapshot` and `restore` take `AppState` apart field by field with no `..`, so a field
//! added to it doesn't compile until it is saved here or left out with a reason.

use super::RunMeta;
use crate::app::{AppState, FxInstance};
use serde::{Deserialize, Serialize};

/// The run, borrowed from the state to be written.
#[derive(Serialize)]
pub struct SnapshotRef<'a> {
    pub params: &'a citygen::GenParams,
    pub city: &'a Option<citygen::City>,
    pub player: &'a Option<crate::walker::Player>,
    pub tick: &'a u64,
    pub chair: &'a Option<crate::clinic::Chair>,
    pub character: &'a game::Character,
    pub world: &'a Option<sim::World>,
    pub interiors: &'a crate::interiors::Interiors,
    pub constructs: &'a crate::constructs::Constructs,
    pub net: &'a crate::net::NetState,
    pub backdoors: &'a crate::backdoors::Backdoors,
    pub ground: &'a game::Ground,
    pub fx: &'a Vec<FxInstance>,
    pub foes: &'a Vec<crate::foes::Foe>,
    pub bots: &'a Vec<crate::bots::BotOut>,
    pub live: &'a Vec<crate::blast::Live>,
    pub shots: &'a u64,
    pub perma_death: &'a bool,
    pub tutorial_off: &'a bool,
    pub log: &'a crate::messages::MessageLog,
    pub journal: &'a crate::credentials::Journal,
    pub mail_seen: &'a usize,
    pub mail_pill: &'a Option<crate::mail_mark::MailPill>,
    pub helix: &'a crate::helix::Helix,
    pub jobs: &'a crate::jobs::Jobs,
    pub missions: &'a crate::missions::Missions,
    pub track: &'a Option<crate::track::Track>,
    pub run: &'a Option<RunMeta>,
    /// The catalogue entries the run made (its keycards): the build's content lacks them.
    pub keycards: Vec<game::Item>,
}

/// The run, read back: the same fields, owned.
#[derive(Deserialize)]
pub struct Snapshot {
    pub params: citygen::GenParams,
    pub city: Option<citygen::City>,
    pub player: Option<crate::walker::Player>,
    pub tick: u64,
    pub chair: Option<crate::clinic::Chair>,
    pub character: game::Character,
    pub world: Option<sim::World>,
    pub interiors: crate::interiors::Interiors,
    pub constructs: crate::constructs::Constructs,
    pub net: crate::net::NetState,
    pub backdoors: crate::backdoors::Backdoors,
    pub ground: game::Ground,
    pub fx: Vec<FxInstance>,
    pub foes: Vec<crate::foes::Foe>,
    pub bots: Vec<crate::bots::BotOut>,
    pub live: Vec<crate::blast::Live>,
    pub shots: u64,
    pub perma_death: bool,
    pub tutorial_off: bool,
    pub log: crate::messages::MessageLog,
    pub journal: crate::credentials::Journal,
    pub mail_seen: usize,
    pub mail_pill: Option<crate::mail_mark::MailPill>,
    pub helix: crate::helix::Helix,
    pub jobs: crate::jobs::Jobs,
    pub missions: crate::missions::Missions,
    pub track: Option<crate::track::Track>,
    pub run: Option<RunMeta>,
    pub keycards: Vec<game::Item>,
}

/// The run as it stands, to be written.
pub fn snapshot(st: &AppState) -> SnapshotRef<'_> {
    let AppState {
        params,
        city,
        player,
        style: _, // content: the build's style pack
        tick,
        dev: _,          // dev switch
        dev_unlocked: _, // dev switch
        rain: _,         // setting
        show_fps: _,     // setting
        effects: _,      // setting: what the window can draw
        screen_fx: _,    // setting
        cord: _,         // setting
        show_eyes: _,    // dev switch
        show_light: _,   // dev switch
        free_chrome: _,  // dev switch
        chair,
        look_at: _, // UI: a popup's request, taken on the map's next frame
        jump_to: _, // UI: the console's request, taken on the map's next frame
        content,
        character,
        people: _, // content: the city's professions and lines
        world,
        interiors,
        constructs,
        net,
        backdoors,
        ground,
        fx,
        foes,
        bots,
        live,
        shots,
        #[cfg(test)]
            force_hit: _, // a test switch
        repaint: _,        // UI: a frame's repaint
        deck_tab: _,       // UI: the deck's remembered tab
        deck_rows: _,      // UI
        deck_kit: _,       // UI
        deck_jobs_open: _, // UI
        deck_jobs_sel: _,  // UI
        deck_map_to: _,    // UI
        deck_sheet: _,     // UI
        deck_talents: _,   // UI
        deck_part: _,      // UI
        build: _,          // UI: the sheet being made before JACK IN
        perma_death,
        tutorial_off,
        log,
        sidebar: _, // setting: the sections shown
        home: _,    // rebuilt from the city by `after_restore`
        journal,
        mail_seen,
        mail_pill,
        helix,
        jobs,
        flavor: _,     // content: the fixers' voice
        book: _,       // content: the story's book
        broadcasts: _, // content
        missions,
        rest: _,      // never Some at a save: no save is taken asleep
        drawn_you: _, // UI: where the last frame drew you
        track,
        atlas: _, // UI: the deck's MAP view
        saves: _, // setting: where this session saves
        run,
        save_due: _,  // the session's: a save asked for
        last_save: _, // the session's: when it last saved
    } = st;
    SnapshotRef {
        params,
        city,
        player,
        tick,
        chair,
        character,
        world,
        interiors,
        constructs,
        net,
        backdoors,
        ground,
        fx,
        foes,
        bots,
        live,
        shots,
        perma_death,
        tutorial_off,
        log,
        journal,
        mail_seen,
        mail_pill,
        helix,
        jobs,
        missions,
        track,
        run,
        keycards: crate::credentials::run_items(content),
    }
}

/// Puts a saved run in place of whatever run `st` held. The settings, dev switches and
/// content stay; the screens' remembered rows start over. `after_restore` then rebuilds
/// what the save leaves out.
pub fn restore(st: &mut AppState, s: Snapshot) {
    let AppState {
        params,
        city,
        player,
        style: _, // content
        tick,
        dev: _,          // dev switch
        dev_unlocked: _, // dev switch
        rain: _,         // setting
        show_fps: _,     // setting
        effects: _,      // setting
        screen_fx: _,    // setting
        cord: _,         // setting
        show_eyes: _,    // dev switch
        show_light: _,   // dev switch
        free_chrome: _,  // dev switch
        chair,
        look_at: _, // UI: never left waiting at a save
        jump_to: _, // UI: never left waiting at a save
        content,
        character,
        people: _, // content
        world,
        interiors,
        constructs,
        net,
        backdoors,
        ground,
        fx,
        foes,
        bots,
        live,
        shots,
        #[cfg(test)]
            force_hit: _, // a test switch
        repaint: _,        // UI
        deck_tab: _,       // UI: `clear_screens` below
        deck_rows: _,      // UI: `clear_screens` below
        deck_kit: _,       // UI: `clear_screens` below
        deck_jobs_open: _, // UI: `clear_screens` below
        deck_jobs_sel: _,  // UI: `clear_screens` below
        deck_map_to: _,    // UI: `clear_screens` below
        deck_sheet: _,     // UI: `clear_screens` below
        deck_talents: _,   // UI: `clear_screens` below
        deck_part: _,      // UI: `clear_screens` below
        build: _,          // UI: the sheet being made before JACK IN
        perma_death,
        tutorial_off,
        log,
        sidebar: _, // setting
        home: _,    // rebuilt by `after_restore`
        journal,
        mail_seen,
        mail_pill,
        helix,
        jobs,
        flavor: _,     // content
        book: _,       // content
        broadcasts: _, // content
        missions,
        rest,
        drawn_you: _, // UI
        track,
        atlas: _, // UI: `clear_screens` below
        saves: _, // setting
        run,
        save_due: _,  // the session's
        last_save: _, // the session's: the opener sets it
    } = st;
    *params = s.params;
    *city = s.city;
    *player = s.player;
    *tick = s.tick;
    *chair = s.chair;
    *character = s.character;
    *world = s.world;
    *interiors = s.interiors;
    *constructs = s.constructs;
    *net = s.net;
    *backdoors = s.backdoors;
    *ground = s.ground;
    *fx = s.fx;
    *foes = s.foes;
    *bots = s.bots;
    *live = s.live;
    *shots = s.shots;
    *perma_death = s.perma_death;
    *tutorial_off = s.tutorial_off;
    *log = s.log;
    *journal = s.journal;
    *mail_seen = s.mail_seen;
    *mail_pill = s.mail_pill;
    *helix = s.helix;
    *jobs = s.jobs;
    *missions = s.missions;
    *track = s.track;
    *run = s.run;
    crate::credentials::put_back(content, s.keycards);
    // Never saved asleep: the run comes back awake.
    *rest = None;
    st.clear_screens();
}

/// Rebuilds what a restored run leaves out: the world's places and labels, the home,
/// and the tracked walk.
pub fn after_restore(st: &mut AppState) {
    if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
        world.rebuild(city, &st.people);
    }
    st.home = st.city.as_ref().and_then(crate::home::HomeRig::find);
    crate::track::refollow(st);
}
