//! When the run saves (design §3): as it starts, when you wake, when a story step ends,
//! from the pause menu, as the window closes and every five minutes of play, each time
//! only if the run can be saved right then; otherwise the last save stands.

use super::{Death, RunHeader, RunId, RunMeta, describe, file};
use crate::app::{AppState, FPS};
use std::time::SystemTime;

/// Frames between the saves five minutes of play make.
pub const EVERY: u64 = 5 * 60 * FPS;
/// Frames the SAVED mark stays up after a save.
pub const MARK: u64 = 3 * FPS;

/// Whether the run can be saved right now: there is a folder to save in and a run in a
/// city that isn't the lot, and you stand in it, alive, awake, with no scene in flight.
/// A call or a feed waiting is part of the run and doesn't stop it, nor does a screen
/// over the map.
pub fn can_save(st: &AppState) -> bool {
    st.saves.is_some()
        && st.run.is_some()
        && st.city.is_some()
        && crate::save::refusal(st).is_none()
        && st.player.is_some()
        && st.net.death.is_none()
        && st.rest.is_none()
        && st.missions.scene.is_none()
}

/// Saves the run, if it can be saved right now: true when it was, and the SAVED mark
/// goes up. A write that fails says why in the log and leaves the last save as it was;
/// the save it was is not asked for again, and the five-minute one waits five minutes.
pub fn save_now(st: &mut AppState) -> bool {
    matches!(try_save(st), Some(Ok(())))
}

/// `save_now`, saying how it went: `None` when the run can't be saved right now (nothing
/// was tried), else the write, and why it failed when it did (the log says so too).
pub fn try_save(st: &mut AppState) -> Option<Result<(), String>> {
    if !can_save(st) {
        return None;
    }
    let dir = st.saves.clone()?;
    st.save_due = false;
    match file::write(&dir, st) {
        Ok(_) => {
            st.last_save = Some((st.tick, SystemTime::now()));
            st.save_failed = None;
            Some(Ok(()))
        }
        Err(e) => {
            st.save_failed = Some(st.tick);
            // The cause underneath (`Permission denied (os error 13)`), not the step
            // that met it.
            let why = e.root_cause().to_string();
            st.warn(format!("couldn't save · {why}"));
            Some(Err(why))
        }
    }
}

/// The map's frame: the save the run asked for, or the one five minutes of play make
/// (counted from the last save, or the last that failed, or else the run's start), once
/// the run can take it. The five-minute one waits out a fight: taken at 2 HP beside
/// black ICE, it is where "load last save" would put you back.
pub fn frame(st: &mut AppState) {
    let since = st
        .last_save
        .map(|(t, _)| t)
        .max(st.save_failed)
        .or(st.run.map(|r| r.started));
    let timed = since.is_some_and(|t| st.tick.saturating_sub(t) >= EVERY);
    if st.save_due || (timed && !fighting(st)) {
        save_now(st);
    }
}

/// A fight is on: a map on turns, or a foe where you stand hunting you.
fn fighting(st: &AppState) -> bool {
    st.net.engaged.is_some()
        || st
            .player
            .as_ref()
            .is_some_and(|p| crate::foes::any_hunting(st, p.place))
}

/// Starts a run in the city just entered and saves it, you standing in it: JACK IN, and
/// the dev start into a fresh city. The lot has no run.
pub fn begin_run(st: &mut AppState) {
    if st.city.is_none() || crate::save::refusal(st).is_some() {
        return;
    }
    st.run = Some(RunMeta {
        id: RunId::fresh(st.tick),
        started: st.tick,
        born: st.net.now(st),
    });
    save_now(st);
}

/// The run in `st` flatlined of `cause` (design §8). With perma death on it is written
/// once more, though the dead can't otherwise save, its header marked with the death,
/// and it is over: from then on it is only in the HALL OF FAME. Without, nothing is
/// written and the last save stands, to go back to. The run's header on disk after
/// either; none without a run or a folder, or when the last write fails (the log says
/// why).
pub fn flatline(st: &mut AppState, cause: &str) -> Option<RunHeader> {
    if !st.perma_death {
        let run = st.run?;
        let dir = st.saves.as_deref()?;
        return file::header(&file::path(dir, run.id)).ok();
    }
    mark_dead(st, cause)?.ok()
}

/// The save on a way out of the run (the pause menu's LOAD GAME and QUIT rows, the app
/// closing): `try_save`, unless the runner has just died in a perma death run, before
/// the map's next frame put up FLATLINED (a quit in the frame you die). That run is
/// marked flatlined instead (`flatline`), so leaving doesn't keep it living.
pub fn save_leaving(st: &mut AppState) -> Option<Result<(), String>> {
    match st.net.death.clone() {
        Some(cause) if st.perma_death => mark_dead(st, &cause).map(|r| r.map(|_| ())),
        _ => try_save(st),
    }
}

/// A perma death run written once more, though the dead can't otherwise save, its
/// header marked with the death, and over. `None` without a run or a folder; why, when
/// the write failed (the log says it too).
fn mark_dead(st: &mut AppState, cause: &str) -> Option<Result<RunHeader, String>> {
    let run = st.run?;
    let written = st.saves.clone().map(|dir| {
        let at = describe::unix_now();
        let death = Death {
            cause: cause.to_string(),
            day: crate::ui::hud::day_at(st.clock_tick()),
            minute: st.minute_of_day(),
            at,
            lasted: st.net.now(st).saturating_sub(run.born),
        };
        let mut header = describe::header(st, Some(death));
        header.saved = at;
        file::write_as(&dir, st, &header).map(|_| header)
    });
    // Over whether or not it was written: nothing saves it again.
    st.run = None;
    match written? {
        Ok(header) => Some(Ok(header)),
        Err(e) => {
            let why = e.root_cause().to_string();
            st.warn(format!("couldn't save · {why}"));
            Some(Err(why))
        }
    }
}

/// Whether the SAVED mark is up: a save in the last three seconds.
pub fn marked(st: &AppState) -> bool {
    st.last_save
        .and_then(|(t, _)| st.tick.checked_sub(t))
        .is_some_and(|since| since < MARK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Screen, ScreenCmd};
    use crate::runs::testutil::{Dir, saving, small_city, story_city};
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::style::{Color, Modifier};
    use std::path::{Path, PathBuf};

    /// The one run in `dir`.
    fn the_file(dir: &Dir) -> PathBuf {
        let runs = crate::runs::list(&dir.0);
        assert_eq!(runs.len(), 1, "one run in {}", dir.0.display());
        runs[0].path.clone()
    }

    /// When the file at `path` says it was saved, in Unix seconds.
    fn stamp(path: &Path) -> u64 {
        file::header(path).unwrap().saved
    }

    /// The file's stamp put back to the first second of 1970, so a save after shows
    /// within the same second.
    fn stamp_old(path: &Path) {
        let mut h = file::header(path).unwrap();
        h.saved = 1;
        file::rewrite_header(path, &h).unwrap();
    }

    /// Frames of one tick on the map with no keys.
    fn frames(st: &mut AppState, map: &mut dyn Screen, n: u64) {
        for _ in 0..n {
            st.tick += 1;
            map.update(st, &[]);
        }
    }

    /// The warnings that say a save failed.
    fn failures(st: &AppState) -> usize {
        st.log
            .iter()
            .filter(|e| e.text.starts_with("couldn't save · "))
            .count()
    }

    #[test]
    fn a_new_run_saves_as_it_starts() {
        let dir = Dir::new("starts");
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        let t = st
            .content
            .template(&game::TemplateId::new("vex"))
            .unwrap()
            .clone();
        let mut build = game::Build::from_template(&st.content, &t);
        build.handle = "Nyx".into();
        st.build = Some(build);
        let mut b = crate::screens::broadcast::Broadcast::new_game(&st, small_city());
        let space = key(KeyCode::Char(' '));
        // The first space shows the broadcast whole; the second is JACK IN.
        b.update(&mut st, &[space]);
        assert!(st.run.is_none() && crate::runs::list(&dir.0).is_empty());
        assert!(matches!(
            b.update(&mut st, &[space]),
            ScreenCmd::Switch { .. }
        ));
        let run = st.run.expect("JACK IN starts a run");
        assert_eq!(run.started, st.tick);
        assert_eq!(run.born, st.net.now(&st));
        let path = the_file(&dir);
        assert_eq!(path, dir.0.join(format!("{run}.run", run = run.id)));
        let h = file::header(&path).unwrap();
        assert_eq!(h.handle, "Nyx", "the build's runner");
        assert!(st.player.is_some(), "saved standing in the city");
        assert_eq!(st.last_save.map(|(t, _)| t), Some(st.tick));
        assert!(!st.save_due, "the story's first step is in that save");
    }

    #[test]
    fn a_new_debug_game_starts_a_run_too() {
        let dir = Dir::new("debug");
        let mut st = AppState::new();
        st.unlock_dev();
        st.saves = Some(dir.0.clone());
        st.params = small_city().params;
        let mut t = crate::screens::title::Title::new(&st, 0);
        t.update(&mut st, &[key(KeyCode::Down)]);
        assert!(matches!(
            t.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        assert!(!st.city.as_ref().unwrap().is_lot());
        let run = st.run.expect("a run in the fresh city");
        assert_eq!(the_file(&dir), dir.0.join(format!("{}.run", run.id)));
    }

    #[test]
    fn waking_and_a_story_step_save() {
        let dir = Dir::new("wake");
        let (mut st, mut map) = saving(&dir, story_city());
        assert!(save_now(&mut st));
        let path = the_file(&dir);
        // A wait, ended with esc: saved on that frame, not before.
        stamp_old(&path);
        let until = crate::rest::now(&st) + 120;
        crate::rest::start(&mut st, crate::rest::Kind::Wait, until);
        assert!(st.rest.is_some(), "waiting");
        frames(&mut st, &mut map, 3);
        assert_eq!(stamp(&path), 1, "no save while waiting");
        st.tick += 1;
        map.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(st.rest.is_none(), "esc ends the wait");
        assert_ne!(stamp(&path), 1, "waking saves");
        assert!(!st.save_due);
        // A story step: its save waits for a frame the map gets to the end of (the
        // welcome feed opens first).
        stamp_old(&path);
        crate::missions::begin(&mut st);
        assert!(st.save_due, "the first step asks for a save");
        frames(&mut st, &mut map, 2);
        assert_ne!(stamp(&path), 1, "the step is saved");
        assert!(!st.save_due);
    }

    #[test]
    fn five_minutes_of_play_save() {
        let dir = Dir::new("five");
        let (mut st, mut map) = saving(&dir, small_city());
        // Never saved yet: five minutes from the run's start.
        st.run.as_mut().unwrap().started = st.tick;
        let start = st.tick;
        let (mut saves, mut last) = (Vec::new(), None);
        for _ in 0..18_000 {
            st.tick += 1;
            map.update(&mut st, &[]);
            let at = st.last_save.map(|(t, _)| t);
            if at != last {
                saves.extend(at);
                last = at;
            }
        }
        assert_eq!(
            saves,
            [start + 9_000, start + 18_000],
            "once every five minutes"
        );
        assert_eq!(EVERY, 9_000);
        the_file(&dir);
    }

    /// Five minutes of play come round mid-fight: on turns, or a foe where you stand
    /// hunting you. That save waits for the fight to end, so "load last save" never puts
    /// you back in it; a save asked for goes as ever.
    #[test]
    fn the_five_minute_save_waits_out_a_fight() {
        use citygen::{Place, StratumKind};
        let street = Place::Stratum(StratumKind::Street);
        let dir = Dir::new("fight");
        let (mut st, _) = saving(&dir, small_city());
        assert!(save_now(&mut st));
        let saved = st.last_save.map(|(t, _)| t);
        // On turns.
        crate::net::engage(&mut st, street, true);
        assert!(st.net.engaged.is_some());
        st.tick += EVERY;
        frame(&mut st);
        assert_eq!(st.last_save.map(|(t, _)| t), saved, "not on turns");
        st.net.engaged = None;
        // A goon beside you, hunting you.
        let at = st.player.as_ref().unwrap().pos;
        let mut goon = crate::foes::Foe::goon(street, citygen::Pos::new(at.x + 3, at.y));
        goon.mode = crate::foes::FoeMode::Hunting { seen: at, lost: 0 };
        st.foes.push(goon);
        st.tick += 1;
        frame(&mut st);
        assert_eq!(st.last_save.map(|(t, _)| t), saved, "not while hunted");
        // A save asked for (a story step, waking) is taken mid-fight all the same.
        st.save_due = true;
        frame(&mut st);
        assert_eq!(st.last_save.map(|(t, _)| t), Some(st.tick));
        let asked = st.last_save;
        // The fight over: the five-minute save goes once it is due again.
        st.foes[0].mode = crate::foes::FoeMode::Idle;
        st.tick += EVERY - 1;
        frame(&mut st);
        assert_eq!(st.last_save, asked);
        st.tick += 1;
        frame(&mut st);
        assert_eq!(
            st.last_save.map(|(t, _)| t),
            Some(st.tick),
            "the fight over"
        );
        // A fight that ran past the five minutes: the save waits, then goes at once.
        st.foes[0].mode = crate::foes::FoeMode::Hunting { seen: at, lost: 0 };
        st.tick += EVERY + 40;
        frame(&mut st);
        let held = st.last_save;
        assert_ne!(held.map(|(t, _)| t), Some(st.tick));
        st.foes[0].mode = crate::foes::FoeMode::Idle;
        st.tick += 1;
        frame(&mut st);
        assert_eq!(st.last_save.map(|(t, _)| t), Some(st.tick), "right after");
    }

    #[test]
    fn a_scene_in_flight_waits_for_the_save() {
        let dir = Dir::new("scene");
        let (mut st, mut map) = saving(&dir, small_city());
        assert!(save_now(&mut st));
        let path = the_file(&dir);
        stamp_old(&path);
        crate::missions::scene::play(&mut st, "waking");
        st.save_due = true;
        frames(&mut st, &mut map, 1);
        assert!(
            crate::missions::scene::running(&st).is_some(),
            "the scene plays"
        );
        assert!(!can_save(&st));
        frames(&mut st, &mut map, 30);
        assert_eq!(stamp(&path), 1, "no save with a scene in flight");
        assert!(st.save_due, "the save still waits");
        crate::missions::scene::skip(&mut st);
        assert!(crate::missions::scene::running(&st).is_none());
        frames(&mut st, &mut map, 1);
        assert_ne!(stamp(&path), 1, "saved once the scene is over");
        assert!(!st.save_due);
    }

    #[test]
    fn a_failed_save_says_so_and_keeps_the_old_one() {
        let dir = Dir::new("failed");
        let (mut st, mut map) = saving(&dir, small_city());
        // The folder to save in is a file.
        let blocked = dir.0.join("not-a-folder");
        std::fs::write(&blocked, b"a file").unwrap();
        st.saves = Some(blocked.clone());
        assert!(!save_now(&mut st));
        // The line names what the system said, as making that folder says it.
        let os = std::fs::create_dir_all(&blocked).unwrap_err().to_string();
        assert!(os.contains("(os error "), "{os}");
        let last = st.log.latest().unwrap().text.clone();
        assert_eq!(last, format!("couldn't save · {os}"));
        assert_eq!(st.last_save, None);
        assert_eq!(std::fs::read(&blocked).unwrap(), b"a file");
        // A save over an older one that fails: the older one stands.
        st.saves = Some(dir.0.clone());
        assert!(save_now(&mut st));
        let path = the_file(&dir);
        let before = std::fs::read(&path).unwrap();
        let saved = st.last_save;
        let tmp = path.with_extension("run.tmp");
        std::fs::create_dir(&tmp).unwrap();
        st.tick += 100;
        assert!(!save_now(&mut st));
        let os = std::fs::File::create(&tmp).unwrap_err().to_string();
        assert!(os.contains("(os error "), "{os}");
        let last = st.log.latest().unwrap().text.clone();
        assert_eq!(last, format!("couldn't save · {os}"));
        assert_eq!(std::fs::read(&path).unwrap(), before, "the old save stands");
        assert_eq!(st.last_save, saved);
        // Asked for again, it is tried once, then not until five minutes on.
        let failed = failures(&st);
        st.save_due = true;
        frames(&mut st, &mut map, 60);
        assert_eq!(failures(&st), failed + 1, "tried once, not every frame");
        frames(&mut st, &mut map, EVERY);
        assert_eq!(failures(&st), failed + 2, "and again five minutes on");
    }

    #[test]
    fn the_lot_never_saves() {
        let dir = Dir::new("lot");
        let mut st = AppState::new();
        st.unlock_dev();
        st.saves = Some(dir.0.clone());
        // The COMBAT start on the street, then on the Net.
        for net in [false, true] {
            let mut t = crate::screens::title::Title::new(&st, 0);
            t.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
            if net {
                t.update(&mut st, &[key(KeyCode::Left)]);
            }
            let ScreenCmd::Switch { to: mut map, .. } = t.update(&mut st, &[key(KeyCode::Enter)])
            else {
                panic!("straight to the lot")
            };
            assert!(st.city.as_ref().unwrap().is_lot());
            assert_eq!(st.run, None, "no run on the lot");
            st.save_due = true;
            frames(&mut st, map.as_mut(), 3);
            st.tick += EVERY;
            frames(&mut st, map.as_mut(), 1);
            begin_run(&mut st);
            assert_eq!(st.run, None, "nor once it is under way");
            assert!(!can_save(&st) && !save_now(&mut st));
            map.update(&mut st, &[key(KeyCode::Char('s'))]);
            assert_eq!(
                st.log.latest().map(|e| e.text.as_str()),
                Some("nothing to save on the lot")
            );
            assert!(std::fs::read_dir(&dir.0).unwrap().next().is_none());
        }
    }

    #[test]
    fn the_way_out_keeps_the_save_from_before_a_feed() {
        use crate::app::Runner;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let dir = Dir::new("feed-exit");
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        let t = st
            .content
            .template(&game::TemplateId::new("vex"))
            .unwrap()
            .clone();
        st.build = Some(game::Build::from_template(&st.content, &t));
        let b = crate::screens::broadcast::Broadcast::new_game(&st, story_city());
        let mut runner = Runner::new(Box::new(b), st);
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let space = key(KeyCode::Char(' '));
        // The feeds waiting in the save on disk.
        let queued = |path: &Path| {
            let mut s = AppState::new();
            let (_, snap, _) = file::read(path, &s.book).unwrap();
            crate::runs::snapshot::restore(&mut s, snap);
            s.missions.feeds.len()
        };
        // The broadcast shown whole, then JACK IN: the run's first save.
        runner.frame(&mut term, &[space]).unwrap();
        runner.frame(&mut term, &[space]).unwrap();
        let path = the_file(&dir);
        assert_eq!(queued(&path), 1, "the welcome feed waits in that save");
        // The map's first frame puts the feed up, off the queue.
        runner.frame(&mut term, &[]).unwrap();
        assert_eq!(runner.depth(), 2, "the feed is up");
        assert!(runner.st.missions.feeds.is_empty());
        let before = std::fs::read(&path).unwrap();
        assert!(!runner.save_on_exit(), "no save on the way out with it up");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the JACK IN save stands"
        );
        assert_eq!(queued(&path), 1);
        // Closed, its step ended: the way out saves.
        runner.frame(&mut term, &[space]).unwrap();
        assert_eq!(runner.depth(), 1);
        assert!(runner.save_on_exit());
        assert_eq!(queued(&path), 0);
    }

    /// The window asks for the exit's save as it closes and again as its loop ends
    /// (`exiting`, where Cmd+Q, the menu's Quit and a logout arrive): one save. A QUIT
    /// the screens asked for saved on its own way out.
    #[test]
    fn the_way_out_saves_once() {
        use crate::app::Runner;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let dir = Dir::new("exit-once");
        let (st, map) = saving(&dir, small_city());
        let mut runner = Runner::new(Box::new(map), st);
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        runner.frame(&mut term, &[]).unwrap();
        assert!(runner.save_on_exit(), "the close button saves");
        let path = the_file(&dir);
        stamp_old(&path);
        runner.frame(&mut term, &[]).unwrap();
        assert!(!runner.save_on_exit(), "the loop's end asks again: nothing");
        assert_eq!(stamp(&path), 1, "not written again");
        // QUIT GAME from the pause menu: saved there, and not again on the way out.
        let dir = Dir::new("exit-quit");
        let (st, map) = saving(&dir, small_city());
        let mut runner = Runner::new(Box::new(map), st);
        runner.frame(&mut term, &[key(KeyCode::Esc)]).unwrap();
        assert_eq!(runner.depth(), 2, "paused");
        let quit = [key(KeyCode::Up), key(KeyCode::Enter)];
        assert!(!runner.frame(&mut term, &quit).unwrap(), "QUIT GAME");
        let path = the_file(&dir);
        stamp_old(&path);
        assert!(!runner.save_on_exit());
        assert_eq!(stamp(&path), 1, "QUIT GAME's save stands");
    }

    #[test]
    fn screens_that_hold_story_say_so() {
        use crate::missions::{FeedView, TrayOffer};
        use crate::screens::popup::Popup;
        let feed = crate::missions::feed::Feed::new(
            FeedView {
                story: None,
                from: "mezz".into(),
                text: vec!["word".into()],
                close: "close".into(),
            },
            0,
        );
        assert!(feed.holds_story(), "a feed, off the queue");
        let mut st = AppState::new();
        st.missions.tray = Some(TrayOffer {
            story: crate::runs::ids::StepRef::new(0, 0),
            from: "doc".into(),
            tab: "doc".into(),
            title: "her tray".into(),
            line: "One piece.".into(),
            items: vec![game::ItemId::new("neural_port")],
            shown: true,
        });
        assert!(Popup::tray(&st).unwrap().holds_story(), "the tray, shown");
        // The rest keep their story on the state, or none.
        let others: Vec<Box<dyn Screen>> = vec![
            Box::new(Popup::jack_out_confirm(0)),
            Box::new(crate::missions::broadcast::Broadcast::new()),
            Box::new(crate::screens::explore::MapScreen::explore(0)),
            Box::new(crate::screens::pause::Pause::new(&st, 0)),
            Box::new(crate::screens::messages::Messages::new(0)),
        ];
        assert!(others.iter().all(|s| !s.holds_story()));
    }

    #[test]
    fn a_flatline_marks_only_a_perma_run() {
        let dir = Dir::new("flatline");
        let (mut st, _) = saving(&dir, small_city());
        st.perma_death = false;
        st.run.as_mut().unwrap().born = st.net.now(&st);
        assert!(save_now(&mut st));
        let path = the_file(&dir);
        let bytes = std::fs::read(&path).unwrap();
        st.net.death = Some("shot".into());
        assert!(!can_save(&st), "the dead don't save");
        // Perma death off: the last save, as it is on disk, untouched.
        let h = flatline(&mut st, "shot").expect("the last save's header");
        assert_eq!(h, file::header(&path).unwrap());
        assert!(h.death.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "nothing written");
        assert!(st.run.is_some(), "the run waits to be reloaded");
        // Perma death on: written once more, the death in its header, and over.
        st.perma_death = true;
        let born = st.run.unwrap().born;
        let h = flatline(&mut st, "shot").expect("written");
        let d = h.death.clone().expect("the death");
        assert_eq!(d.cause, "shot");
        assert_eq!(d.minute, st.minute_of_day());
        assert_eq!(d.day, crate::ui::hud::day_at(st.clock_tick()));
        assert_eq!(d.lasted, st.net.now(&st) - born);
        assert_eq!(file::header(&path).unwrap(), h);
        assert!(st.run.is_none());
        assert!(crate::runs::living(&dir.0).is_empty());
        assert_eq!(crate::runs::dead(&dir.0).len(), 1);
        // No run left: nothing.
        assert!(flatline(&mut st, "shot").is_none());
        // No folder: nothing written, either way.
        let (mut st, _) = saving(&dir, small_city());
        st.saves = None;
        for perma in [true, false] {
            st.perma_death = perma;
            assert!(flatline(&mut st, "shot").is_none());
        }
        assert_eq!(crate::runs::list(&dir.0).len(), 1);
    }

    #[test]
    fn s_on_the_map_saves_the_run() {
        let dir = Dir::new("s-key");
        let (mut st, mut map) = saving(&dir, small_city());
        assert!(matches!(
            map.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Stay
        ));
        the_file(&dir);
        assert!(marked(&st));
    }

    #[test]
    fn nothing_saves_without_a_folder() {
        let dir = Dir::new("no-folder");
        let (mut st, mut map) = saving(&dir, small_city());
        st.saves = None;
        assert!(!can_save(&st) && !save_now(&mut st));
        st.save_due = true;
        frames(&mut st, &mut map, 2);
        assert!(crate::runs::list(&dir.0).is_empty());
        assert_eq!(failures(&st), 0, "no warning: nothing was tried");
    }

    #[test]
    fn the_mark_shows_for_three_seconds() {
        let dir = Dir::new("mark");
        let (mut st, mut map) = saving(&dir, small_city());
        frames(&mut st, &mut map, 1);
        let status = |st: &AppState, map: &dyn Screen| {
            let buf = crate::screens::shots::draw_stack(st, &[map], (120, 40));
            let text = buffer_text(&buf);
            let y = text
                .lines()
                .position(|l| l.starts_with(" on foot · "))
                .expect("the status row");
            (buf, y as u16, text.lines().nth(y).unwrap().to_string())
        };
        let (_, _, row) = status(&st, &map);
        assert!(!row.contains("SAVED"), "{row:?}");
        assert!(save_now(&mut st));
        let (buf, y, row) = status(&st, &map);
        let place = row.find(" :: SAVED").unwrap_or_else(|| panic!("{row:?}"));
        assert!(
            row[..place].trim_start().starts_with("on foot · "),
            "right after the place: {row:?}"
        );
        let x = row[..place].chars().count() as u16;
        assert_eq!(
            buf[(x + 1, y)].fg,
            Color::Rgb(100, 110, 125),
            "the :: faint"
        );
        let s = &buf[(x + 4, y)];
        assert_eq!((s.symbol(), s.fg), ("S", Color::Rgb(0, 204, 184)));
        assert!(s.modifier.contains(Modifier::BOLD));
        st.tick += 89;
        assert!(status(&st, &map).2.contains(":: SAVED"), "up for 89 frames");
        st.tick += 1;
        let (_, _, row) = status(&st, &map);
        assert!(!row.contains("SAVED"), "gone at 90: {row:?}");
    }
}
