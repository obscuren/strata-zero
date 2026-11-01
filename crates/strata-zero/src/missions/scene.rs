//! Scenes (scenes spec §2): scripted stretches played on the map. The game acts for
//! the player, walking them and the visitors tile by tile, typing a keypad, opening a
//! door, while the lines play out; then it hands control back. `film` and `bubbles`
//! draw them; this is the runtime.
//!
//! Every timing is the scene's data, in seconds, run on the clock tick: a scene pauses
//! with the world. `space` (or `enter`) is next, `esc` skips, every other key is
//! swallowed. A scene never stalls: each beat ends on its own time or on arrival, a
//! mark that resolves to nothing never blocks, and a fight skips the scene.
//!
//! Adding a beat (spec §14): its arm in `start` (what happens as it begins), `ended`
//! (when it is over) and `finish` (its end state, on time, on `space` or in a skip).

use super::call::{self, TICKS_PER_SECOND};
use super::{
    Beat, Look, Mark, PadCode, PadResult, Scene, YOU, effects, fill, marks, role_name, visitors,
};
use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use crate::screens::keypad::Keypad;
use citygen::{Place, Pos, RoomKind};
use crossterm::event::{KeyCode, KeyEvent};

/// A scene playing: where it stands in its beats, and what it shows.
#[derive(Clone, Debug)]
pub struct SceneRun {
    /// The mission and step that played it; `usize::MAX` from the console.
    mission: usize,
    step: usize,
    id: String,
    scene: Scene,
    /// The beat in progress.
    beat: usize,
    /// The clock tick it began.
    began: u64,
    stage: Option<String>,
    line: Option<Line>,
    /// Walks on their way: the beat's, and any a `wait: false` left going beside.
    walks: Vec<WalkRun>,
    keypad: Option<PadRun>,
    /// The doors drawn open.
    doors: Vec<(Place, Pos)>,
    /// The tile the camera holds; `None` follows the player.
    camera: Option<(Place, Pos)>,
}

/// The line being said.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Shown and logged: `as` if set, else the role's name, your handle for `you`.
    pub label: String,
    /// The role speaking, or `you`.
    pub who: String,
    /// Spoken `as` someone else: the speaker's colour isn't shown.
    pub hidden: bool,
    pub text: String,
    /// What the line points at: its `at` mark, else the speaker.
    pub anchor: Option<(Place, Pos)>,
    /// The clock tick it began to type, or was shown whole.
    pub began: u64,
    /// The scene's typing speed; infinite once `space` has shown the line whole.
    pub chars_per_tick: f32,
}

/// An actor on the way, a tile every `step_ticks`.
#[derive(Clone, Debug)]
struct WalkRun {
    who: String,
    place: Place,
    /// The tiles still to step, the goal last.
    path: Vec<Pos>,
    goal: Pos,
    /// The clock tick of the next step.
    next: u64,
    step_ticks: u64,
}

/// A keypad typed on: a digit every `step_ticks`, the verdict with the last, held
/// `hold_ticks`.
#[derive(Clone, Debug)]
struct PadRun {
    /// None when the mark found no flat's door: the time passes all the same.
    pad: Option<Keypad>,
    /// The digits still to type.
    digits: Vec<u8>,
    open: bool,
    /// The clock tick of the next digit.
    next: u64,
    step_ticks: u64,
    hold_ticks: u64,
    verdict_at: Option<u64>,
}

impl SceneRun {
    pub fn id(&self) -> &str {
        &self.id
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    pub fn look(&self) -> Look {
        self.scene.look
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    pub fn title(&self) -> &str {
        &self.scene.title
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    /// The last stage line.
    pub fn stage(&self) -> Option<&str> {
        self.stage.as_deref()
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    pub fn line(&self) -> Option<&Line> {
        self.line.as_ref()
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    /// The keypad on screen.
    pub fn pad(&self) -> Option<&Keypad> {
        self.keypad.as_ref().and_then(|p| p.pad.as_ref())
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    pub fn doors(&self) -> &[(Place, Pos)] {
        &self.doors
    }

    /// The beat in progress, cloned so that running it may change the run.
    fn current(&self) -> Option<Beat> {
        self.scene.beats.get(self.beat).cloned()
    }
}

impl Line {
    /// Clock ticks the whole line takes to type.
    fn type_ticks(&self) -> u64 {
        (self.text.chars().count() as f32 / self.chars_per_tick).ceil() as u64
    }

    /// Still typing at clock tick `now`.
    pub fn typing(&self, now: u64) -> bool {
        now < self.began.saturating_add(self.type_ticks())
    }

    #[allow(dead_code)] // the film and the bubbles draw it from the next tasks
    /// The text typed by clock tick `now`.
    pub fn shown(&self, now: u64) -> &str {
        if !self.typing(now) {
            return &self.text;
        }
        let n = (now.saturating_sub(self.began) as f32 * self.chars_per_tick) as usize;
        self.text
            .char_indices()
            .nth(n)
            .map_or(&self.text, |(i, _)| &self.text[..i])
    }
}

/// The scene playing.
pub fn running(st: &AppState) -> Option<&SceneRun> {
    st.missions.scene.as_ref()
}

/// The tile a scene holds the camera on, while it does.
pub fn camera(st: &AppState) -> Option<(Place, Pos)> {
    running(st).and_then(|r| r.camera)
}

/// Dev console: plays the scene next, for no mission's step, so its end moves nothing on.
pub fn play(st: &mut AppState, id: &str) {
    st.missions
        .scenes
        .push_front((usize::MAX, usize::MAX, id.to_string()));
}

/// A scene takes every key while it plays: `space` and `enter` are next, `esc` skips,
/// the rest do nothing. False when none plays.
pub fn key(st: &mut AppState, k: &KeyEvent) -> bool {
    if st.missions.scene.is_none() {
        return false;
    }
    match k.code {
        KeyCode::Char(' ') | KeyCode::Enter => next(st),
        KeyCode::Esc => skip(st),
        _ => {}
    }
    true
}

/// The scene's beat in the story's tick: a queued scene starts when it may, a fight
/// skips the one playing, its walks and keypad step, and the beat in progress moves on
/// once it has ended. A scene that ends lets the next start in the same tick, so no
/// call or feed slips in between two; the passes bound a book that chains them.
pub fn tick(st: &mut AppState) {
    for _ in 0..super::PASSES {
        if st.missions.scene.is_none() && !start_next(st) {
            return;
        }
        if call::busy(st) {
            skip(st);
            continue;
        }
        // Started and over in one go: the next may start.
        let Some(mut run) = st.missions.scene.take() else {
            continue;
        };
        let now = st.clock_tick();
        step_walks(st, &mut run, now);
        step_pad(st, &mut run, now);
        let over = match run.current() {
            Some(beat) if ended(st, &run, &beat, now) => !advance(st, &mut run),
            Some(_) => false,
            None => true,
        };
        if over {
            close(st, run);
            continue;
        }
        st.missions.scene = Some(run);
        return;
    }
}

/// `esc`, a fight or the console: every beat left applies its end state at once (the
/// lines not yet said logged, actors on their goals, arrivals, leaves and lights done),
/// then the scene ends.
pub fn skip(st: &mut AppState) {
    let Some(mut run) = st.missions.scene.take() else {
        return;
    };
    if let Some(beat) = run.current() {
        finish(st, &mut run, &beat);
    }
    run.beat += 1;
    while let Some(beat) = run.current() {
        start(st, &mut run, &beat);
        finish(st, &mut run, &beat);
        run.beat += 1;
    }
    close(st, run);
}

/// `space`: a typing line shows whole; else the beat in progress ends now, in its end
/// state, and the scene moves on.
fn next(st: &mut AppState) {
    let Some(mut run) = st.missions.scene.take() else {
        return;
    };
    let now = st.clock_tick();
    if let Some(line) = run.line.as_mut().filter(|l| l.typing(now)) {
        // Whole from now, so a hold counts from here.
        line.began = now;
        line.chars_per_tick = f32::INFINITY;
        st.missions.scene = Some(run);
        return;
    }
    if advance(st, &mut run) {
        st.missions.scene = Some(run);
    } else {
        close(st, run);
    }
}

/// Starts the first queued scene unless a fight or a run keeps it waiting; a call up
/// goes to voicemail. False when none was waiting, or it must wait.
fn start_next(st: &mut AppState) -> bool {
    if call::busy(st) {
        return false;
    }
    let Some((mission, step, id)) = st.missions.scenes.pop_front() else {
        return false;
    };
    let Some(scene) = st.book.scenes.get(&id).cloned() else {
        // Never so in a checked book; the step must not wait on it for ever.
        super::end(st, mission, step);
        return true;
    };
    let began = st.clock_tick();
    st.missions.scene = Some(SceneRun {
        mission,
        step,
        id,
        scene,
        beat: 0,
        began,
        stage: None,
        line: None,
        walks: Vec::new(),
        keypad: None,
        doors: Vec::new(),
        camera: None,
    });
    // With the scene up, the next queued call keeps waiting behind it.
    call::voicemail(st);
    if let Some(mut run) = st.missions.scene.take() {
        if chain(st, &mut run) {
            st.missions.scene = Some(run);
        } else {
            close(st, run);
        }
    }
    true
}

/// Starts the beat at `run.beat`, and each after it that is over as it begins, so
/// instant beats chain in one tick. False when no beat is left.
fn chain(st: &mut AppState, run: &mut SceneRun) -> bool {
    let now = st.clock_tick();
    while let Some(beat) = run.current() {
        run.began = now;
        start(st, run, &beat);
        if !ended(st, run, &beat, now) {
            return true;
        }
        finish(st, run, &beat);
        run.beat += 1;
    }
    false
}

/// The beat in progress is over: its end state, then the next. False after the last.
fn advance(st: &mut AppState, run: &mut SceneRun) -> bool {
    if let Some(beat) = run.current() {
        finish(st, run, &beat);
    }
    run.beat += 1;
    chain(st, run)
}

/// The scene is over: a walk left going ends on its goal, the doors and the camera go
/// with the run, and the step that played it has ended.
fn close(st: &mut AppState, run: SceneRun) {
    for w in &run.walks {
        put(st, &w.who, w.place, w.goal);
    }
    super::end(st, run.mission, run.step);
}

/// What a beat does as it begins.
fn start(st: &mut AppState, run: &mut SceneRun, beat: &Beat) {
    let now = st.clock_tick();
    match beat {
        Beat::Say {
            who,
            text,
            label,
            at,
            ..
        } => {
            let line = Line {
                label: match label {
                    Some(l) => fill(st, l),
                    None if who == YOU => st.character.handle.clone(),
                    None => role_name(st, who),
                },
                who: who.clone(),
                hidden: label.is_some(),
                text: fill(st, text),
                anchor: at
                    .as_ref()
                    .and_then(|m| marks::resolve(st, m, None))
                    .or_else(|| spot(st, who)),
                began: now,
                chars_per_tick: run.scene.pace.chars / TICKS_PER_SECOND as f32,
            };
            st.story(format!("`{}` · \u{201c}{}\u{201d}", line.label, line.text));
            run.line = Some(line);
        }
        Beat::Stage(text) => run.stage = Some(fill(st, text)),
        Beat::Walk { who, to, pace, .. } => {
            let step_ticks = ticks(pace.unwrap_or(run.scene.pace.walk));
            walk(st, run, who, to, step_ticks);
        }
        Beat::Wait(_) => {}
        Beat::Keypad {
            door,
            code,
            result,
            keys,
            hold,
        } => {
            let p = pad(st, door, *code, *result, ticks(*keys), ticks(*hold));
            run.keypad = Some(p);
        }
        Beat::Door { at, open } => {
            if let Some(door) = marks::resolve(st, at, None) {
                run.doors.retain(|d| *d != door);
                if *open {
                    run.doors.push(door);
                }
            }
        }
        Beat::Camera(Mark::You) => run.camera = None,
        Beat::Camera(mark) => {
            if let Some(tile) = marks::resolve(st, mark, None) {
                run.camera = Some(tile);
            }
        }
        Beat::Arrive { role, at } => effects::arrive(st, role, at),
        Beat::Leave(role) => {
            run.walks.retain(|w| &w.who != role);
            visitors::leave(st, role);
        }
        Beat::Lights(level) => effects::lights(st, *level),
    }
}

/// Whether the beat in progress, begun at `run.began`, is over at clock tick `now`.
fn ended(_st: &AppState, run: &SceneRun, beat: &Beat, now: u64) -> bool {
    match beat {
        // Typed, then held; with no hold set anywhere, only `space` ends it.
        Beat::Say { hold, .. } => run.line.as_ref().is_none_or(|l| {
            !l.typing(now)
                && hold.or(run.scene.pace.hold).is_some_and(|h| {
                    now >= l
                        .began
                        .saturating_add(l.type_ticks())
                        .saturating_add(ticks(h))
                })
        }),
        Beat::Walk { who, wait, .. } => !wait || !run.walks.iter().any(|w| &w.who == who),
        Beat::Wait(secs) => now >= run.began.saturating_add(ticks(*secs)),
        Beat::Keypad { .. } => run.keypad.as_ref().is_none_or(|p| {
            p.verdict_at
                .is_some_and(|at| now >= at.saturating_add(p.hold_ticks))
        }),
        Beat::Stage(_)
        | Beat::Door { .. }
        | Beat::Camera(_)
        | Beat::Arrive { .. }
        | Beat::Leave(_)
        | Beat::Lights(_) => true,
    }
}

/// A beat's end state, as it ends on time, on `space` or in a skip.
fn finish(st: &mut AppState, run: &mut SceneRun, beat: &Beat) {
    match beat {
        Beat::Say { .. } => run.line = None,
        // The walk the beat waits on arrives; one left going beside the beats goes on.
        Beat::Walk {
            who, wait: true, ..
        } => {
            if let Some(i) = run.walks.iter().position(|w| &w.who == who) {
                let w = run.walks.remove(i);
                put(st, &w.who, w.place, w.goal);
            }
        }
        Beat::Keypad { .. } => run.keypad = None,
        Beat::Walk { .. }
        | Beat::Stage(_)
        | Beat::Wait(_)
        | Beat::Door { .. }
        | Beat::Camera(_)
        | Beat::Arrive { .. }
        | Beat::Leave(_)
        | Beat::Lights(_) => {}
    }
}

/// Seconds of a scene's data as clock ticks, at least one.
fn ticks(secs: f32) -> u64 {
    (secs * TICKS_PER_SECOND as f32).round().max(1.0) as u64
}

/// Sets an actor walking to the mark: a path over the floor, or straight there when the
/// mark lies on another map or no path leads to it. An actor not on the map, or a mark
/// that resolves to nothing, walks nowhere.
fn walk(st: &mut AppState, run: &mut SceneRun, who: &str, to: &Mark, step_ticks: u64) {
    run.walks.retain(|w| w.who != who);
    let walker = (who != YOU).then_some(who);
    let Some((place, goal)) = marks::resolve(st, to, walker) else {
        if st.dev {
            st.warn(format!("scene · {to:?} not found"));
        }
        return;
    };
    let Some((there, from)) = spot(st, who) else {
        if st.dev {
            st.warn(format!("scene · {who} is not on the map"));
        }
        return;
    };
    if there != place {
        put(st, who, place, goal);
        return;
    }
    match path(st, who, place, from, goal) {
        Some(path) if !path.is_empty() => run.walks.push(WalkRun {
            who: who.to_string(),
            place,
            path,
            goal,
            next: st.clock_tick().saturating_add(step_ticks),
            step_ticks,
        }),
        Some(_) => {}
        None => put(st, who, place, goal),
    }
}

/// The tiles from `from` to `goal`, four ways over the place's walkable tiles, both ends
/// allowed to be unwalkable (a visitor on the couch) and tiles under other actors
/// blocked; `from` itself left out.
fn path(st: &AppState, who: &str, place: Place, from: Pos, goal: Pos) -> Option<Vec<Pos>> {
    let map = MapRef::for_place(
        st.city.as_ref()?,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    )?;
    let tiles = citygen::pathing::astar(map.size(), from, goal, |_, to| {
        (to == goal || (map.walkable(to) && !taken(st, who, place, to))).then_some(1)
    })?;
    Some(tiles[1..].to_vec())
}

/// Each walk steps the tiles whose time has come, the player seeing from each. A tile
/// someone has stepped into since finds a new way round, or the walker goes to the goal.
fn step_walks(st: &mut AppState, run: &mut SceneRun, now: u64) {
    for w in &mut run.walks {
        while now >= w.next {
            let Some(&to) = w.path.first() else {
                break;
            };
            if taken(st, &w.who, w.place, to) {
                let round = spot(st, &w.who)
                    .and_then(|(_, here)| path(st, &w.who, w.place, here, w.goal))
                    .unwrap_or_default();
                if round.first().is_none_or(|p| taken(st, &w.who, w.place, *p)) {
                    w.path.clear();
                    put(st, &w.who, w.place, w.goal);
                    break;
                }
                w.path = round;
                continue;
            }
            w.path.remove(0);
            put(st, &w.who, w.place, to);
            w.next = w.next.saturating_add(w.step_ticks);
        }
    }
    run.walks.retain(|w| !w.path.is_empty());
}

/// The keypad's next digits as their time comes, the verdict with the last.
fn step_pad(st: &AppState, run: &mut SceneRun, now: u64) {
    let Some(p) = run.keypad.as_mut() else {
        return;
    };
    while p.verdict_at.is_none() && now >= p.next {
        if !p.digits.is_empty() {
            let d = p.digits.remove(0);
            if let Some(pad) = p.pad.as_mut() {
                pad.show_digit(d);
            }
        }
        if p.digits.is_empty() {
            p.verdict_at = Some(p.next);
            // The flash's glitch runs on the frame tick, as the real pad's does.
            if let Some(pad) = p.pad.as_mut() {
                pad.show_verdict(p.open, st.tick);
            }
        } else {
            p.next = p.next.saturating_add(p.step_ticks);
        }
    }
}

/// The keypad of the flat whose door the mark names, to type the code the beat asks
/// for: the journal's for that flat (`0000` when it knows none), or the beat's own.
fn pad(
    st: &AppState,
    door: &Mark,
    code: PadCode,
    result: PadResult,
    step_ticks: u64,
    hold_ticks: u64,
) -> PadRun {
    let flat = marks::resolve(st, door, None).and_then(|(place, pos)| {
        let Place::Floor { building, level } = place else {
            return None;
        };
        let (_, floor) = st.interiors.floor(building, level)?;
        let room = floor
            .door_of(pos)
            .filter(|r| matches!(r.kind, RoomKind::Flat | RoomKind::Home))?;
        Some((building, level, room.number))
    });
    let code = match code {
        // A pad takes four digits.
        PadCode::Code(n) => n % 10_000,
        PadCode::Journal => flat
            .and_then(|(b, level, n)| st.journal.code_for(b, level, n))
            .map_or(0, |c| c.code),
    };
    PadRun {
        pad: flat.map(|(b, level, n)| Keypad::scripted(st, b, level, n)),
        digits: format!("{code:04}").bytes().map(|c| c - b'0').collect(),
        open: result == PadResult::Open,
        next: st.clock_tick().saturating_add(step_ticks),
        step_ticks,
        hold_ticks,
        verdict_at: None,
    }
}

/// Where an actor stands: you, or a visitor on the map.
fn spot(st: &AppState, who: &str) -> Option<(Place, Pos)> {
    if who == YOU {
        st.player.as_ref().map(|p| (p.place, p.pos))
    } else {
        st.missions
            .visitors
            .iter()
            .find(|v| v.role == who)
            .map(|v| (v.place, v.pos))
    }
}

/// Another actor than `who` stands on the tile.
fn taken(st: &AppState, who: &str, place: Place, pos: Pos) -> bool {
    let you = who != YOU
        && st
            .player
            .as_ref()
            .is_some_and(|p| p.place == place && p.pos == pos);
    you || st
        .missions
        .visitors
        .iter()
        .any(|v| v.role != who && v.place == place && v.pos == pos)
}

/// Moves an actor: you, seeing from the new tile, or a visitor still on the map.
fn put(st: &mut AppState, who: &str, place: Place, pos: Pos) {
    if who != YOU {
        if let Some(v) = st.missions.visitors.iter_mut().find(|v| v.role == who) {
            (v.place, v.pos) = (place, pos);
        }
        return;
    }
    let bonus = st.sight_bonus();
    let minute = st.minute_of_day();
    let Some(p) = st.player.as_mut() else {
        return;
    };
    (p.place, p.pos) = (place, pos);
    if let Some(city) = st.city.as_ref()
        && let Some(map) = MapRef::for_place(city, &st.interiors, &st.constructs, place, minute)
        && let Some(p) = st.player.as_mut()
    {
        p.update_fov(&map, bonus);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::on_street;
    use super::super::{Book, Effect, Mark, begin, effects, marks, tick, visitors};
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::jobs::testutil::state_home;
    use crate::render::Visibility;
    use crate::render::viewport::Camera;
    use crate::screens::explore::MapScreen;
    use crate::walker::Player;
    use citygen::{GenParams, Generator, SizePreset, StratumKind};
    use crossterm::event::KeyCode;

    const SPACE: KeyCode = KeyCode::Char(' ');
    /// Typing a character a tick, lines waiting for space.
    const SLOW: &str = "Pace(type: 30, walk: 0.5, hold: None)";

    /// Two visitors, and a mission whose first step plays scene "s" (these beats at
    /// this pace) and whose second waits on its end.
    fn book(pace: &str, beats: &str) -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "castellan": Role(name: "Castellan", title: "stranger", pick: Visitor,
                        face: None, tint: Some((255, 200, 80))),
                    "seren": Role(name: "Seren", title: "netrunner", pick: Visitor,
                        face: None, tint: Some((255, 0, 200))),
                }},
                faces: {{
                    "silhouette": Face(rows: ["................", "................"], eye: (0, 0, 0)),
                }},
                missions: [Mission(id: "m", title: "M", from: "castellan", steps: [
                    Step(id: "play", when: NewGame, then: [Scene("s")]),
                    Step(id: "after", when: Ended("play"), then: [Log("after the scene")]),
                ])],
                scenes: {{
                    "s": Scene(title: "Test", look: Film, pace: {pace}, beats: [{beats}]),
                }},
            )"#
        ))
        .unwrap()
    }

    /// Puts the player on a mark, nothing seen yet.
    fn stand(st: &mut AppState, mark: &Mark) {
        let (place, pos) = marks::resolve(st, mark, None).unwrap();
        st.player = Some(Player::at(place, pos));
    }

    /// Seed 8 moved in with the test's book, outside the flat's door, the story begun:
    /// the scene waits its turn.
    fn queued(pace: &str, beats: &str) -> AppState {
        let mut st = state_home();
        st.character.handle = "VEX".into();
        st.book = book(pace, beats);
        stand(&mut st, &Mark::HomeOutside);
        begin(&mut st);
        assert_eq!(st.missions.scenes.len(), 1);
        st
    }

    /// The same, the scene started.
    fn playing(pace: &str, beats: &str) -> AppState {
        let mut st = queued(pace, beats);
        tick(&mut st);
        assert!(running(&st).is_some(), "the scene starts");
        st
    }

    /// Runs the clock on `n` ticks, the story's beat after each.
    fn run_ticks(st: &mut AppState, n: u64) {
        for _ in 0..n {
            if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
                world.advance(city, world.tick() + 1);
            }
            tick(st);
        }
    }

    fn press(st: &mut AppState, code: KeyCode) -> bool {
        key(st, &crate::screens::testutil::key(code))
    }

    /// The beat in progress.
    fn beat(st: &AppState) -> usize {
        running(st).expect("a scene runs").beat
    }

    fn me(st: &AppState) -> (Place, Pos) {
        let p = st.player.as_ref().unwrap();
        (p.place, p.pos)
    }

    fn at(st: &AppState, mark: &Mark) -> (Place, Pos) {
        marks::resolve(st, mark, None).unwrap()
    }

    /// How many log lines read exactly `text`.
    fn logged(st: &AppState, text: &str) -> usize {
        st.log.iter().filter(|e| e.text == text).count()
    }

    fn ended(st: &AppState) -> bool {
        logged(st, "after the scene") == 1 && st.missions.runs[0].ended.contains(&0)
    }

    #[test]
    fn a_held_line_moves_on_by_itself() {
        let mut st = playing(
            "Pace(type: 30, walk: 0.3, hold: None)",
            r#"Say(who: "castellan", text: "abc", hold: Some(1.0)),
            Say(who: "castellan", text: "next")"#,
        );
        let line = running(&st).unwrap().line().unwrap().clone();
        let t = line.began;
        assert_eq!((line.shown(t), line.shown(t + 1)), ("", "a"));
        assert!(line.typing(t + 2) && !line.typing(t + 3));
        assert_eq!(line.shown(t + 3), "abc");
        // Three ticks of typing and a second held.
        run_ticks(&mut st, 3 + 30 - 1);
        assert_eq!(beat(&st), 0);
        run_ticks(&mut st, 1);
        assert_eq!(beat(&st), 1);
        assert_eq!(running(&st).unwrap().line().unwrap().text, "next");
    }

    #[test]
    fn a_line_without_hold_waits_for_space() {
        let mut st = playing(
            SLOW,
            r#"Say(who: "castellan", text: "Wait."), Say(who: "castellan", text: "On.")"#,
        );
        run_ticks(&mut st, 10_000);
        assert_eq!(beat(&st), 0);
        let now = st.clock_tick();
        assert!(!running(&st).unwrap().line().unwrap().typing(now));
        assert!(press(&mut st, SPACE));
        assert_eq!(beat(&st), 1);
    }

    #[test]
    fn space_finishes_typing_first() {
        let text = "A line long enough to type for a while.";
        let mut st = playing(
            SLOW,
            &format!(r#"Say(who: "castellan", text: "{text}"), Say(who: "seren", text: "Next.")"#),
        );
        run_ticks(&mut st, 5);
        let now = st.clock_tick();
        let line = running(&st).unwrap().line().unwrap();
        assert!(line.typing(now) && line.shown(now).len() < text.len());
        assert!(press(&mut st, SPACE));
        let line = running(&st).unwrap().line().unwrap();
        assert_eq!(beat(&st), 0);
        assert_eq!(line.shown(now), text);
        assert!(!line.typing(now));
        assert!(press(&mut st, SPACE));
        assert_eq!(beat(&st), 1);
    }

    #[test]
    fn a_walk_takes_its_pace() {
        let mut st = playing(
            SLOW,
            r#"Walk(who: "you", to: HomeInside(2)), Say(who: "you", text: "In.")"#,
        );
        let goal = at(&st, &Mark::HomeInside(2));
        let from = me(&st).1;
        let steps = running(&st).unwrap().walks[0].path.len() as u64;
        assert_eq!(steps, from.manhattan(goal.1) as u64, "a path, not a jump");
        // Half a second a tile: 15 ticks.
        run_ticks(&mut st, steps * 15 - 1);
        assert_ne!(me(&st), goal);
        assert_eq!(beat(&st), 0);
        run_ticks(&mut st, 1);
        assert_eq!(me(&st), goal);
        assert_eq!(beat(&st), 1);
        let vis = st.player.as_ref().unwrap().vis();
        for mark in [Mark::HomeInside(1), Mark::HomeInside(2)] {
            assert_eq!(
                vis.get(at(&st, &mark).1),
                Some(&Visibility::Visible),
                "{mark:?}"
            );
        }
    }

    #[test]
    fn space_ends_a_walk_at_its_goal() {
        let mut st = playing(
            SLOW,
            r#"Walk(who: "you", to: HomeInside(2)), Say(who: "you", text: "In.")"#,
        );
        let goal = at(&st, &Mark::HomeInside(2));
        run_ticks(&mut st, 5);
        assert_ne!(me(&st), goal);
        assert!(press(&mut st, SPACE));
        assert_eq!(me(&st), goal);
        assert_eq!(beat(&st), 1);
        assert!(running(&st).unwrap().walks.is_empty());
        let vis = st.player.as_ref().unwrap().vis();
        assert_eq!(vis.get(goal.1), Some(&Visibility::Visible));
    }

    #[test]
    fn the_keypad_types_and_touches_nothing() {
        let mut st = playing(
            SLOW,
            r#"Keypad(door: HomeDoor, code: Journal, result: Denied, keys: 0.1, hold: 0.5),
            Say(who: "you", text: "WTF?")"#,
        );
        let h = st.home.unwrap().home;
        let (b, level, number) = (h.building, h.level, h.number);
        let code = st.journal.code_for(b, level, number).unwrap().code;
        let want: Vec<u8> = format!("{code:04}").bytes().map(|c| c - b'0').collect();
        let codes = st.journal.codes().len();
        let state = st.journal.keypad(b, level, number);
        let typed = |st: &AppState| running(st).unwrap().pad().unwrap().typed().to_vec();
        assert!(typed(&st).is_empty());
        // A digit every three ticks.
        for n in 1..=4 {
            run_ticks(&mut st, 2);
            assert_eq!(typed(&st), want[..n - 1], "before digit {n}");
            run_ticks(&mut st, 1);
            assert_eq!(typed(&st), want[..n], "digit {n}");
        }
        // The verdict held for half a second.
        run_ticks(&mut st, 15 - 1);
        assert_eq!(beat(&st), 0);
        assert!(running(&st).unwrap().pad().is_some());
        run_ticks(&mut st, 1);
        assert_eq!(beat(&st), 1);
        assert!(running(&st).unwrap().pad().is_none());
        assert_eq!(st.journal.codes().len(), codes);
        assert_eq!(st.journal.keypad(b, level, number), state);
        let now = st.net.now(&st);
        assert_eq!(st.journal.keypad_locked_until(b, level, number, now), None);
    }

    #[test]
    fn esc_skips_to_the_end_state() {
        let mut st = queued(
            SLOW,
            r#"Say(who: "castellan", text: "One."),
            Walk(who: "seren", to: HomeOutside),
            Say(who: "seren", text: "Two."),
            Leave("castellan"),
            Lights(1.0)"#,
        );
        let couch = at(&st, &Mark::HomeFurniture(citygen::Furniture::Couch));
        let rig = at(&st, &Mark::HomeRig);
        visitors::arrive(&mut st, "castellan", couch);
        visitors::arrive(&mut st, "seren", rig);
        st.missions.lights = Some(0.45);
        stand(&mut st, &Mark::HomeInside(1));
        tick(&mut st);
        assert!(running(&st).is_some());
        assert!(press(&mut st, KeyCode::Esc));
        assert!(running(&st).is_none());
        assert_eq!(st.missions.visitors.len(), 1);
        let out = at(&st, &Mark::HomeOutside);
        assert_eq!(visitors::at(&st, out.0, out.1).unwrap().role, "seren");
        assert_eq!(st.missions.lights, None);
        assert_eq!(logged(&st, "Castellan · \u{201c}One.\u{201d}"), 1);
        assert_eq!(logged(&st, "Seren · \u{201c}Two.\u{201d}"), 1);
        assert!(ended(&st));
    }

    #[test]
    fn keys_do_nothing_but_space_enter_esc() {
        let mut st = queued(
            SLOW,
            r#"Say(who: "castellan", text: "Stay."), Say(who: "castellan", text: "Go.")"#,
        );
        let mut map = MapScreen::walking(&mut st);
        stand(&mut st, &Mark::HomeInside(1));
        assert!(matches!(map.update(&mut st, &[]), ScreenCmd::Stay));
        assert!(running(&st).is_some());
        let here = me(&st);
        let tab = st.deck_tab;
        let k = crate::screens::testutil::key;
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Char('h'),
            KeyCode::Char('w'),
            KeyCode::Tab,
            KeyCode::Char('1'),
            KeyCode::Char('q'),
            KeyCode::Char('x'),
            KeyCode::Char('?'),
            KeyCode::Char(':'),
        ] {
            assert!(key(&mut st, &k(code)), "{code:?} is the scene's");
            let cmd = map.update(&mut st, &[k(code)]);
            assert!(matches!(cmd, ScreenCmd::Stay), "{code:?} opened something");
            assert_eq!(me(&st), here, "{code:?} moved you");
            assert_eq!(st.deck_tab, tab);
            assert_eq!(beat(&st), 0, "{code:?} moved the scene on");
            assert!(!map.help && !map.is_looking());
        }
        // Enter is next, as space is: the typing first, then the line.
        map.update(&mut st, &[k(KeyCode::Enter)]);
        assert_eq!(beat(&st), 0);
        map.update(&mut st, &[k(KeyCode::Enter)]);
        assert_eq!(beat(&st), 1);
        map.update(&mut st, &[k(KeyCode::Esc)]);
        assert!(running(&st).is_none());
        assert!(
            matches!(map.update(&mut st, &[]), ScreenCmd::Stay),
            "no pause menu"
        );
        assert!(!press(&mut st, SPACE), "no scene, no key taken");
    }

    #[test]
    fn a_scene_waits_out_a_fight() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Later.")"#);
        let place = me(&st).0;
        crate::net::engage(&mut st, place, true);
        assert!(st.net.engaged.is_some());
        run_ticks(&mut st, 30);
        assert!(running(&st).is_none());
        assert_eq!(st.missions.scenes.len(), 1);
        assert!(!press(&mut st, SPACE));
        st.net.engaged = None;
        tick(&mut st);
        assert!(running(&st).is_some());
        assert!(st.missions.scenes.is_empty());
    }

    #[test]
    fn a_fight_mid_scene_skips_it() {
        let mut st = playing(
            SLOW,
            r#"Say(who: "castellan", text: "One."), Say(who: "castellan", text: "Two.")"#,
        );
        let place = me(&st).0;
        crate::net::engage(&mut st, place, true);
        tick(&mut st);
        assert!(running(&st).is_none());
        assert_eq!(logged(&st, "Castellan · \u{201c}Two.\u{201d}"), 1);
        assert!(ended(&st));
    }

    #[test]
    fn a_scene_off_home_never_stalls() {
        let mut st = queued(
            "Pace(type: 30, walk: 0.3, hold: Some(0.5))",
            r#"Walk(who: "you", to: HomeInside(2)),
            Arrive(role: "seren", at: HomeRig),
            Say(who: "seren", text: "Nobody home."),
            Say(who: "you", text: "No.", hold: Some(1.0)),
            Camera(HomeDoor),
            Keypad(door: HomeDoor, code: Journal, result: Open, keys: 0.1, hold: 0.1),
            Door(at: HomeDoor, open: true)"#,
        );
        let b = st.home.unwrap().home.building;
        let below = st.city.as_ref().unwrap().portals(b).first().unwrap().1;
        st.home = None;
        on_street(&mut st, below);
        let before = me(&st);
        tick(&mut st);
        assert!(running(&st).is_some());
        // Typing (12 and 3 characters a tick each), the holds (15 and 30 ticks), the
        // keypad's four digits (3 ticks each) and its hold (3).
        run_ticks(&mut st, 12 + 15 + 3 + 30 + 4 * 3 + 3);
        assert!(running(&st).is_none(), "the scene ends");
        assert!(ended(&st));
        assert!(st.missions.visitors.is_empty());
        assert_eq!(me(&st), before);
    }

    #[test]
    fn feeds_and_calls_wait_for_the_scene() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Hold on.")"#);
        let mut map = MapScreen::walking(&mut st);
        stand(&mut st, &Mark::HomeInside(1));
        assert!(matches!(map.update(&mut st, &[]), ScreenCmd::Stay));
        assert!(running(&st).is_some());
        let feed = Effect::Feed {
            from: "castellan".into(),
            text: vec!["A feed.".into()],
            close: "ok".into(),
        };
        let call = Effect::Call {
            from: "castellan".into(),
            lines: vec!["A call.".into()],
        };
        effects::apply(&mut st, 0, 1, &feed);
        effects::apply(&mut st, 0, 1, &call);
        for _ in 0..3 {
            assert!(
                matches!(map.update(&mut st, &[]), ScreenCmd::Stay),
                "no feed"
            );
            run_ticks(&mut st, 10);
        }
        assert!(st.missions.call.is_none(), "no call rings");
        assert_eq!((st.missions.calls.len(), st.missions.feeds.len()), (1, 1));
        // The scene over, the call rings and the feed opens.
        let k = crate::screens::testutil::key;
        map.update(&mut st, &[k(KeyCode::Esc)]);
        assert!(running(&st).is_none());
        let up = st.missions.call.as_ref().expect("the call rings");
        assert_eq!(up.lines, vec!["A call.".to_string()]);
        assert!(matches!(map.update(&mut st, &[]), ScreenCmd::Push { .. }));
    }

    #[test]
    fn a_call_up_when_a_scene_starts_goes_to_voicemail() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Now.")"#);
        let call = Effect::Call {
            from: "seren".into(),
            lines: vec!["Pick up.".into()],
        };
        effects::apply(&mut st, 0, 1, &call);
        assert!(st.missions.call.is_some());
        tick(&mut st);
        assert!(running(&st).is_some());
        assert!(st.missions.call.is_none());
        let mail = st.journal.mails.iter().find(|m| m.subject == "voicemail");
        assert_eq!(
            mail.map(|m| m.body.clone()),
            Some(vec!["Pick up.".to_string()])
        );
    }

    #[test]
    fn a_new_city_ends_the_scene() {
        let mut st = playing(
            SLOW,
            r#"Camera(HomeInside(2)), Door(at: HomeDoor, open: true),
            Say(who: "castellan", text: "Stay.")"#,
        );
        assert!(camera(&st).is_some());
        let run = running(&st).unwrap();
        assert_eq!((run.look(), run.title()), (Look::Film, "Test"));
        assert_eq!(run.doors(), [at(&st, &Mark::HomeDoor)]);
        st.enter_city(
            Generator::run(GenParams {
                seed: 9,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        assert!(running(&st).is_none());
        assert!(camera(&st).is_none());
        assert!(st.missions.scenes.is_empty());
        tick(&mut st);
        assert!(running(&st).is_none());
        assert!(!press(&mut st, SPACE));
    }

    #[test]
    fn lines_are_logged_as_they_start() {
        let mut st = playing(
            SLOW,
            r#"Stage("The lock clicks."),
            Say(who: "castellan", as: Some("A voice"), at: Some(HomeDoor), text: "Come in, {you}."),
            Say(who: "you", text: "Who?"),
            Say(who: "castellan", text: "Sit.")"#,
        );
        let run = running(&st).unwrap();
        assert_eq!(run.stage(), Some("The lock clicks."));
        let line = run.line().unwrap();
        assert_eq!(
            (line.label.as_str(), line.who.as_str(), line.hidden),
            ("A voice", "castellan", true)
        );
        assert_eq!(line.text, "Come in, VEX.");
        assert_eq!(line.anchor, Some(at(&st, &Mark::HomeDoor)));
        assert_eq!(logged(&st, "A voice · \u{201c}Come in, VEX.\u{201d}"), 1);
        assert!(st.log.iter().all(|e| !e.text.contains("The lock clicks")));
        assert_eq!(logged(&st, "VEX · \u{201c}Who?\u{201d}"), 0, "not yet");
        assert!(press(&mut st, SPACE) && press(&mut st, SPACE));
        assert_eq!(logged(&st, "VEX · \u{201c}Who?\u{201d}"), 1);
        let line = running(&st).unwrap().line().unwrap();
        assert_eq!((line.label.as_str(), line.hidden), ("VEX", false));
        assert_eq!(line.anchor, Some(me(&st)));
        assert!(press(&mut st, SPACE) && press(&mut st, SPACE));
        assert_eq!(logged(&st, "Castellan · \u{201c}Sit.\u{201d}"), 1);
        // The log keeps the label marked, as a call's lines are.
        let last = st.log.latest().unwrap();
        assert!(!last.marks.is_empty());
        // Skipped, nothing is logged twice.
        assert!(press(&mut st, KeyCode::Esc));
        assert_eq!(logged(&st, "Castellan · \u{201c}Sit.\u{201d}"), 1);
        assert_eq!(logged(&st, "A voice · \u{201c}Come in, VEX.\u{201d}"), 1);
    }

    #[test]
    fn the_camera_holds_a_mark_then_follows_you() {
        let mut st = queued(
            SLOW,
            r#"Camera(Role("castellan")), Say(who: "you", text: "Look."),
            Camera(You), Say(who: "you", text: "Back.")"#,
        );
        let mut map = MapScreen::walking(&mut st);
        // On the street, Castellan thirty tiles off.
        let b = st.home.unwrap().home.building;
        let below = st.city.as_ref().unwrap().portals(b).first().unwrap().1;
        on_street(&mut st, below);
        let far = (Place::Stratum(StratumKind::Street), below.offset(30, 0));
        visitors::arrive(&mut st, "castellan", far);
        map.update(&mut st, &[]);
        assert_eq!(camera(&st), Some(far));
        map.update(&mut st, &[]);
        // Centred on him, kept inside the map as the camera always is.
        let view = map.map_area();
        let mut want = Camera::default();
        want.target(far.1);
        want.clamp(st.city.as_ref().unwrap().size, view.width, view.height);
        assert_eq!((map.cam.tx, map.cam.ty), (want.tx, want.ty));
        let zone = i32::from(view.width) / 6;
        assert!(map.cam.tx - below.x > zone, "the camera left you");
        assert!(press(&mut st, SPACE) && press(&mut st, SPACE));
        assert_eq!(beat(&st), 3);
        assert_eq!(camera(&st), None);
        map.update(&mut st, &[]);
        assert!(map.cam.tx - below.x <= zone, "back on you");
    }

    #[test]
    fn the_console_opens_over_a_scene_and_skips_it() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Stay.")"#);
        st.unlock_dev();
        let mut map = MapScreen::walking(&mut st);
        stand(&mut st, &Mark::HomeInside(1));
        map.update(&mut st, &[]);
        assert!(running(&st).is_some());
        let k = crate::screens::testutil::key;
        let mut keys = vec![k(KeyCode::Char(':'))];
        keys.extend("story skip".chars().map(|c| k(KeyCode::Char(c))));
        keys.push(k(KeyCode::Enter));
        map.update(&mut st, &keys);
        assert!(running(&st).is_none());
        assert!(ended(&st));
    }

    #[test]
    fn play_queues_a_scene_for_no_step() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Again.")"#);
        tick(&mut st);
        assert!(press(&mut st, KeyCode::Esc));
        assert!(ended(&st));
        play(&mut st, "s");
        tick(&mut st);
        assert_eq!(running(&st).map(SceneRun::id), Some("s"));
        assert!(press(&mut st, KeyCode::Esc));
        assert!(running(&st).is_none());
        assert_eq!(logged(&st, "after the scene"), 1, "ending it ends nothing");
    }
}
