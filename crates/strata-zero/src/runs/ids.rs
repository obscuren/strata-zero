//! Story positions by id. The engine keeps a mission by its place in the book and a
//! step by its place in the mission; a save writes their ids instead, translated
//! through the book in scope (`with_book`), so a build that adds, moves or drops a step
//! reads an old run where it was. Reading against a book that lacks an id records a
//! line for it, and `drop_lost` drops what held it.

use super::snapshot::Snapshot;
use crate::missions::{Book, END, Missions, Run, RunState};
use serde::de::Error as _;
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// A step of a mission, by their places in the book: where something the story put
/// out came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepRef {
    pub mission: usize,
    pub step: usize,
}

impl StepRef {
    pub const fn new(mission: usize, step: usize) -> StepRef {
        StepRef { mission, step }
    }

    /// What one read from a save is when this build's book lacks its mission or step:
    /// `drop_lost` drops whatever holds it before the run is put back.
    const LOST: StepRef = StepRef::new(usize::MAX, usize::MAX);

    fn lost(self) -> bool {
        self == StepRef::LOST
    }
}

const NO_BOOK: &str = "no book in scope";
const NOT_IN_BOOK: &str = "a story position the book lacks";

/// The book's ids by place: each mission's id, and its steps' ids.
struct Ids(Vec<(String, Vec<String>)>);

impl Ids {
    fn of(book: &Book) -> Ids {
        Ids(book
            .missions
            .iter()
            .map(|m| (m.id.clone(), m.steps.iter().map(|s| s.id.clone()).collect()))
            .collect())
    }

    fn mission(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|(m, _)| m == id)
    }

    fn step(&self, m: usize, id: &str) -> Option<usize> {
        self.0.get(m)?.1.iter().position(|s| s == id)
    }

    fn names(&self, r: StepRef) -> Option<(&str, &str)> {
        let (m, steps) = self.0.get(r.mission)?;
        Some((m, steps.get(r.step)?))
    }
}

/// The book a save is written or read against, and what reading has found.
struct Scope {
    ids: Ids,
    found: Found,
}

impl Scope {
    /// The line for what this build no longer has, once.
    fn lose(&mut self, line: String) {
        if !self.found.lost.contains(&line) {
            self.found.lost.push(line);
        }
    }

    /// A step read by its ids; `LOST` when the book lacks either, said (a step once
    /// reading is done, `Found::settle`).
    fn resolve(&mut self, mission: &str, step: &str) -> StepRef {
        let Some(m) = self.ids.mission(mission) else {
            self.lose(format!("a mission ({mission})"));
            return StepRef::LOST;
        };
        let Some(s) = self.ids.step(m, step) else {
            self.found
                .gone
                .push((mission.to_string(), step.to_string()));
            return StepRef::LOST;
        };
        StepRef::new(m, s)
    }
}

/// What reading a run against a book found.
#[derive(Debug, Default)]
struct Found {
    /// A line for each mission or step the run had and the book lacks.
    lost: Vec<String>,
    /// The book's missions the run had no run for, by place.
    fresh: Vec<usize>,
    /// Steps the book lacks that something in the run pointed at (a call, a feed, a
    /// mail), by mission and step id, as met.
    gone: Vec<(String, String)>,
    /// Steps the book lacks that the run had ended: the player had finished them, so
    /// nothing of theirs goes with them and nothing is said.
    finished: BTreeSet<(String, String)>,
}

impl Found {
    /// Once the whole run is read: a line for each step gone that it hadn't ended. The
    /// runs are read after what points into them, so this waits for them.
    fn settle(&mut self) {
        for (mission, step) in std::mem::take(&mut self.gone) {
            if self.finished.contains(&(mission.clone(), step.clone())) {
                continue;
            }
            let line = format!("a step of {mission} ({step})");
            if !self.lost.contains(&line) {
                self.lost.push(line);
            }
        }
    }
}

thread_local! {
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

/// Runs `f` with `book` in scope: every `StepRef` and `Missions.runs` written or read
/// inside it goes by that book's ids. The scope before is back when it returns.
pub fn with_book<R>(book: &Book, f: impl FnOnce() -> R) -> R {
    /// Puts the scope before back, even when `f` panics.
    struct Restore(Option<Scope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let was = self.0.take();
            SCOPE.with(|s| *s.borrow_mut() = was);
        }
    }
    let scope = Scope {
        ids: Ids::of(book),
        found: Found::default(),
    };
    let _restore = Restore(SCOPE.with(|s| s.replace(Some(scope))));
    f()
}

/// A run's body (its snapshot, as MessagePack) read against `book` and made whole by
/// `drop_lost`: the snapshot, and a line for each mission or step it had that the book
/// lacks. The one way in for a snapshot: nothing a gone step held reaches the engine.
pub(crate) fn read_snapshot(
    book: &Book,
    body: &[u8],
) -> Result<(Snapshot, Vec<String>), rmp_serde::decode::Error> {
    let (snap, found) = reading(book, || rmp_serde::from_slice::<Snapshot>(body));
    let mut snap = snap?;
    let lost = drop_lost(&mut snap, book, found);
    Ok((snap, lost))
}

/// `f` run with `book` in scope, and what reading inside it found.
fn reading<R>(book: &Book, f: impl FnOnce() -> R) -> (R, Found) {
    with_book(book, || {
        let out = f();
        let mut found = SCOPE.with(|s| {
            s.borrow_mut()
                .as_mut()
                .map(|s| std::mem::take(&mut s.found))
                .unwrap_or_default()
        });
        found.settle();
        (out, found)
    })
}

/// The ids on disk.
#[derive(Serialize, Deserialize)]
struct Named {
    mission: String,
    step: String,
}

impl Serialize for StepRef {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let named = SCOPE.with(|scope| {
            let scope = scope.borrow();
            let scope = scope.as_ref().ok_or(NO_BOOK)?;
            let (mission, step) = scope.ids.names(*self).ok_or(NOT_IN_BOOK)?;
            Ok::<_, &str>(Named {
                mission: mission.to_string(),
                step: step.to_string(),
            })
        });
        named.map_err(S::Error::custom)?.serialize(s)
    }
}

impl<'de> Deserialize<'de> for StepRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let Named { mission, step } = Named::deserialize(d)?;
        SCOPE
            .with(|scope| {
                let mut scope = scope.borrow_mut();
                let scope = scope.as_mut().ok_or(NO_BOOK)?;
                Ok::<_, &str>(scope.resolve(&mission, &step))
            })
            .map_err(D::Error::custom)
    }
}

/// `Missions.runs` as a save keeps it: a map from each mission's id to its run, the
/// run's steps by id (`end` past the last). Read against a book, a mission it lacks is
/// dropped and said, and each mission it has that the save didn't is left for
/// `drop_lost` to start as `begin` does. A run that never ran waits on the book's first
/// step; one under way whose step the book lacks is done.
pub mod runs {
    use super::*;

    /// One mission's run on disk.
    #[derive(Serialize, Deserialize)]
    struct Saved {
        at: String,
        since: u64,
        state: RunState,
        objectives: Vec<String>,
        ended: Vec<String>,
        began: Option<u64>,
    }

    pub fn serialize<S: Serializer>(runs: &[Run], s: S) -> Result<S::Ok, S::Error> {
        SCOPE.with(|scope| {
            let scope = scope.borrow();
            let scope = scope.as_ref().ok_or_else(|| S::Error::custom(NO_BOOK))?;
            let mut map = s.serialize_map(Some(runs.len()))?;
            for (m, run) in runs.iter().enumerate() {
                let (id, steps) = scope
                    .ids
                    .0
                    .get(m)
                    .ok_or_else(|| S::Error::custom(NOT_IN_BOOK))?;
                let name = |i: usize| {
                    steps
                        .get(i)
                        .cloned()
                        .ok_or_else(|| S::Error::custom(NOT_IN_BOOK))
                };
                let Run {
                    at,
                    since,
                    state,
                    objectives,
                    ended,
                    began,
                } = run;
                let saved = Saved {
                    at: if *at >= steps.len() {
                        END.to_string()
                    } else {
                        name(*at)?
                    },
                    since: *since,
                    state: *state,
                    objectives: objectives.clone(),
                    ended: ended.iter().map(|&i| name(i)).collect::<Result<_, _>>()?,
                    began: *began,
                };
                map.serialize_entry(id, &saved)?;
            }
            map.end()
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Run>, D::Error> {
        let saved = BTreeMap::<String, Saved>::deserialize(d)?;
        SCOPE.with(|scope| {
            let mut scope = scope.borrow_mut();
            let scope = scope.as_mut().ok_or_else(|| D::Error::custom(NO_BOOK))?;
            // The story hasn't begun: no runs, whatever the book holds.
            if saved.is_empty() {
                return Ok(Vec::new());
            }
            let mut runs: Vec<Option<Run>> = vec![None; scope.ids.0.len()];
            for (id, run) in saved {
                let Some(m) = scope.ids.mission(&id) else {
                    scope.lose(format!("a mission ({id})"));
                    continue;
                };
                runs[m] = Some(read_run(scope, m, &id, run));
            }
            Ok(runs
                .into_iter()
                .enumerate()
                .map(|(m, run)| {
                    run.unwrap_or_else(|| {
                        scope.found.fresh.push(m);
                        // `drop_lost` puts the run `begin` would make in its place.
                        Run {
                            at: 0,
                            since: 0,
                            state: RunState::Waiting,
                            objectives: Vec::new(),
                            ended: BTreeSet::new(),
                            began: None,
                        }
                    })
                })
                .collect())
        })
    }

    /// Mission `m`'s run, its steps found by id.
    fn read_run(scope: &mut Scope, m: usize, id: &str, run: Saved) -> Run {
        let Saved {
            at,
            since,
            state,
            objectives,
            ended,
            began,
        } = run;
        // An ended step the book lacks goes from the set unsaid: the player finished it.
        let ended = ended
            .into_iter()
            .filter_map(|s| {
                let i = scope.ids.step(m, &s);
                if i.is_none() {
                    scope.found.finished.insert((id.to_string(), s));
                }
                i
            })
            .collect();
        let last = scope.ids.0[m].1.len();
        let (at, state) = match state {
            // Never run (only running a step moves `at`, and neither runs one): it waits
            // on the book's first step, whatever that is now.
            RunState::Waiting | RunState::Uncast => (0, state),
            _ if at == END => (last, state),
            _ => match scope.ids.step(m, &at) {
                Some(i) => (i, state),
                // Under way on a step the book lacks: it can't go on, so it is done.
                None => {
                    scope.lose(format!("a step of {id} ({at})"));
                    (last, RunState::Done { at: since })
                }
            },
        };
        Run {
            at,
            since,
            state,
            objectives,
            ended,
            began,
        }
    }
}

/// A run read against `book`, made whole: each of the book's missions the run had no
/// run for starts as `begin` starts one, and whatever holds a step the book lacks goes
/// (a call, a feed, a scene, the news, the tray, a mail's link to its step, the track).
/// The lines saying what is gone.
fn drop_lost(s: &mut Snapshot, book: &Book, found: Found) -> Vec<String> {
    let Found {
        lost,
        fresh,
        gone: _,     // said in `lost` by `settle`
        finished: _, // unsaid
    } = found;
    // The minute `NetState::now` gives once the run is back: the world's clock.
    let now = s.world.as_ref().map_or(s.tick, |w| w.tick()) / sim::TICKS_PER_MINUTE;
    clear_lost(
        &mut s.missions,
        &mut s.journal,
        &mut s.track,
        book,
        &fresh,
        now,
    );
    // the home city's story, set aside while the run is in another city
    if let Some(away) = s.away.as_mut() {
        let now = away
            .world
            .as_ref()
            .map_or(now * sim::TICKS_PER_MINUTE, |w| w.tick())
            / sim::TICKS_PER_MINUTE;
        clear_lost(
            &mut away.missions,
            &mut away.journal,
            &mut away.track,
            book,
            &fresh,
            now,
        );
    }
    lost
}

/// What a story (`missions`, its `journal`'s mails, the `track`) held of steps the book
/// lacks, dropped; the book's missions the run had no run for (`fresh`) started as
/// `begin` would at minute `now`.
fn clear_lost(
    missions: &mut Missions,
    journal: &mut crate::credentials::Journal,
    track: &mut Option<crate::track::Track>,
    book: &Book,
    fresh: &[usize],
    now: u64,
) {
    for &m in fresh {
        if let (Some(mission), Some(run)) = (book.missions.get(m), missions.runs.get_mut(m)) {
            *run = crate::missions::fresh_run(book, &missions.cast, mission, now);
        }
    }
    let gone = |r: &Option<StepRef>| r.is_some_and(StepRef::lost);
    // Every field named, so one added to `Missions` is weighed here.
    let Missions {
        cast: _,
        runs: _, // read by id above
        bodies: _,
        feeds,
        call,
        calls,
        begun: _,
        visitors: _,
        lights: _,
        scenes,
        scene: _, // never saved
        news,
        news_ends,
        daze: _,
        asides: _, // never saved
        unsealed: _,
        read: _,
        releasing: _,
        notes: _,
        tray,
        tab: _,
    } = missions;
    feeds.retain(|f| !gone(&f.story));
    if call.as_ref().is_some_and(|c| gone(&c.story)) {
        *call = None;
    }
    calls.retain(|c| !gone(&c.story));
    scenes.retain(|(r, _)| !gone(r));
    if news.as_ref().is_some_and(|n| gone(&n.story)) {
        *news = None;
    }
    news_ends.retain(|r| !r.lost());
    if tray.as_ref().is_some_and(|t| t.story.lost()) {
        *tray = None;
    }
    for mail in &mut journal.mails {
        if gone(&mail.story) {
            mail.story = None;
        }
    }
    if track
        .as_ref()
        .is_some_and(|t| matches!(t.source, crate::track::Source::Story(r) if r.lost()))
    {
        *track = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::missions::{Book, Call, FeedView, RunState, TrayOffer};
    use crate::runs::file::{read, write};
    use crate::runs::snapshot::{after_restore, restore};
    use crate::runs::testutil::{Dir, running};
    use crate::screens::shots::seren_has_written;
    use game::ItemId;
    use rmpv::Value;
    use std::collections::BTreeSet;
    use std::io::Read;
    use std::path::Path;

    /// The shipped mission `id`'s place in the book.
    fn mission_at(book: &Book, id: &str) -> usize {
        book.missions.iter().position(|m| m.id == id).unwrap()
    }

    /// Step `id` of mission `m`'s place in it.
    fn step_at(book: &Book, m: usize, id: &str) -> usize {
        book.missions[m]
            .steps
            .iter()
            .position(|s| s.id == id)
            .unwrap()
    }

    /// The body of the run at `path`, as a plain value.
    fn body_of(path: &Path) -> Value {
        let bytes = std::fs::read(path).unwrap();
        let header_end = 10 + u32::from_le_bytes(bytes[6..10].try_into().unwrap()) as usize;
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(&bytes[header_end..])
            .read_to_end(&mut raw)
            .unwrap();
        rmpv::decode::read_value(&mut raw.as_slice()).unwrap()
    }

    /// The entry under `key` in the map `v`.
    fn field<'a>(v: &'a Value, key: &str) -> &'a Value {
        v.as_map()
            .unwrap_or_else(|| panic!("a map holding {key}: {v}"))
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no {key} in {v}"))
    }

    /// The morning after, waiting on the doctor's tray, every doctor step before it ended.
    fn at_the_tray() -> (AppState, usize, usize) {
        let mut st = running(seren_has_written());
        let d = mission_at(&st.book, "doctor");
        let tray = step_at(&st.book, d, "tray");
        let run = &mut st.missions.runs[d];
        run.at = tray;
        run.state = RunState::Open;
        run.ended = (0..tray).collect();
        (st, d, tray)
    }

    #[test]
    fn story_positions_are_written_by_id() {
        let dir = Dir::new("by-id");
        let st = running(seren_has_written());
        let m = mission_at(&st.book, "morning_after");
        let run = &st.missions.runs[m];
        let steps = &st.book.missions[m].steps;
        assert!(run.at < steps.len(), "the morning after is under way");
        let path = write(&dir.0, &st).unwrap();
        let body = body_of(&path);
        let runs = field(field(&body, "missions"), "runs");
        assert!(
            runs.is_map(),
            "the runs are a map by mission id, not an array in book order: {runs}"
        );
        let morning = field(runs, "morning_after");
        assert_eq!(
            field(morning, "at").as_str(),
            Some(steps[run.at].id.as_str())
        );
        let ended: Vec<&str> = field(morning, "ended")
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().expect("an ended step by its id"))
            .collect();
        let want: Vec<&str> = run.ended.iter().map(|&i| steps[i].id.as_str()).collect();
        assert_eq!(ended, want);
        // A done mission's `at` is past its last step.
        let drop = field(runs, "the_drop");
        assert_eq!(field(drop, "at").as_str(), Some("end"));
        // A mail's story link is its mission and step by id.
        let mails = field(field(&body, "journal"), "mails").as_array().unwrap();
        let link = mails
            .iter()
            .map(|m| field(m, "story"))
            .find(|s| !s.is_nil())
            .expect("a mail the story sent");
        assert!(field(link, "mission").is_str(), "{link}");
        assert!(field(link, "step").is_str(), "{link}");
    }

    #[test]
    fn a_step_added_before_where_you_are_doesnt_rewind() {
        let dir = Dir::new("added");
        let (st, d, tray) = at_the_tray();
        let ended: BTreeSet<String> = st.missions.runs[d]
            .ended
            .iter()
            .map(|&i| st.book.missions[d].steps[i].id.clone())
            .collect();
        assert!(
            st.journal
                .mails
                .iter()
                .any(|m| m.story.is_some_and(|r| r.mission == d)),
            "a mail from the doctor's steps"
        );
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        let mut added = book.missions[d].steps[0].clone();
        added.id = "added".to_string();
        book.missions[d].steps.insert(0, added);

        let (_, s, lost) = read(&path, &book).expect("the run reads");
        assert!(lost.is_empty(), "{lost:?}");
        let run = &s.missions.runs[d];
        assert_eq!(run.at, tray + 1);
        assert_eq!(book.missions[d].steps[run.at].id, "tray");
        let back: BTreeSet<String> = run
            .ended
            .iter()
            .map(|&i| book.missions[d].steps[i].id.clone())
            .collect();
        assert_eq!(back, ended, "every ended step is still ended, by id");
        assert!(!run.ended.contains(&0), "the new step has not ended");
        // The mails' links to the doctor's steps moved with them.
        for (was, now) in st.journal.mails.iter().zip(&s.journal.mails) {
            match was.story {
                Some(r) if r.mission == d => {
                    assert_eq!(now.story, Some(StepRef::new(d, r.step + 1)));
                }
                other => assert_eq!(now.story, other),
            }
        }
    }

    #[test]
    fn a_mission_gone_from_the_book_is_dropped_and_said() {
        let dir = Dir::new("gone");
        let st = running(seren_has_written());
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        book.missions.retain(|m| m.id != "ledger");
        let (_, s, lost) = read(&path, &book).expect("the run still reads");
        assert!(lost.iter().any(|l| l.contains("ledger")), "{lost:?}");
        assert_eq!(s.missions.runs.len(), book.missions.len());
        let mut b = AppState::new();
        b.book = book;
        restore(&mut b, s);
        after_restore(&mut b, lost);
        assert!(
            b.log
                .iter()
                .any(|e| e.text.starts_with("lost in the update · ") && e.text.contains("ledger")),
            "the log says so"
        );
        crate::missions::tick(&mut b);
    }

    #[test]
    fn a_mission_gone_from_the_middle_moves_the_rest_up() {
        let dir = Dir::new("middle");
        let st = running(seren_has_written());
        let house = mission_at(&st.book, "house_call");
        let morning = mission_at(&st.book, "morning_after");
        assert!(house < morning);
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        book.missions.remove(house);
        let (_, s, lost) = read(&path, &book).expect("the run still reads");
        assert!(
            lost.contains(&"a mission (house_call)".to_string()),
            "{lost:?}"
        );
        assert_eq!(
            s.missions.runs[morning - 1],
            st.missions.runs[morning],
            "the morning after moved up a place, as it was"
        );
        for (was, now) in st.journal.mails.iter().zip(&s.journal.mails) {
            let want = was.story.and_then(|r| match r.mission {
                m if m == house => None,
                m if m > house => Some(StepRef::new(m - 1, r.step)),
                _ => Some(r),
            });
            assert_eq!(now.story, want, "{}", was.subject);
        }
        assert!(
            st.journal
                .mails
                .iter()
                .any(|m| m.story.is_some_and(|r| r.mission == house)),
            "a house call mail lost its link"
        );
    }

    #[test]
    fn a_mission_new_to_the_book_starts_as_begin_starts_it() {
        let dir = Dir::new("new");
        let st = running(seren_has_written());
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        let mut added = book.missions.last().unwrap().clone();
        added.id = "added".to_string();
        book.missions.insert(0, added);
        let (_, s, lost) = read(&path, &book).expect("the run reads");
        assert!(lost.is_empty(), "{lost:?}");
        assert_eq!(s.missions.runs.len(), book.missions.len());
        let now = st.net.now(&st);
        assert_eq!(
            s.missions.runs[0],
            crate::missions::fresh_run(&book, &s.missions.cast, &book.missions[0], now)
        );
        assert_eq!(s.missions.runs[1..], st.missions.runs[..]);
    }

    #[test]
    fn a_mission_not_begun_waits_on_the_books_first_step() {
        use crate::missions::{Effect, Mission, Step, Trigger};
        let dir = Dir::new("not-begun");
        let st = running(seren_has_written());
        let w = st
            .missions
            .runs
            .iter()
            .position(|r| r.state == RunState::Waiting)
            .expect("a mission not yet begun");
        let id = st.book.missions[w].id.clone();
        let path = write(&dir.0, &st).unwrap();
        // Its first step renamed away, and a mission new to the book waiting on it done.
        let mut book = st.book.clone();
        book.missions[w].steps.remove(0);
        book.missions.push(Mission {
            id: "after".into(),
            title: "After".into(),
            from: book.missions[w].from.clone(),
            steps: vec![Step {
                id: "go".into(),
                when: Trigger::MissionDone {
                    mission: id.clone(),
                    minutes: 0,
                },
                then: vec![Effect::Log("went".into())],
            }],
        });

        let (_, s, lost) = read(&path, &book).expect("the run reads");
        assert!(lost.is_empty(), "nothing it had is lost: {lost:?}");
        let run = &s.missions.runs[w];
        assert_eq!((run.at, run.state), (0, RunState::Waiting), "{id}");
        assert!(run.ended.is_empty());
        let mut b = AppState::new();
        b.book = book;
        restore(&mut b, s);
        after_restore(&mut b, lost);
        crate::missions::tick(&mut b);
        let after = b.book.missions.len() - 1;
        assert_eq!(
            b.missions.runs[after].state,
            RunState::Waiting,
            "nothing waiting on {id} being done fired"
        );
        assert!(b.log.iter().all(|e| e.text != "went"));
    }

    #[test]
    fn a_step_gone_ends_its_mission_and_drops_what_held_it() {
        let dir = Dir::new("step");
        let (mut st, d, tray) = at_the_tray();
        let here = StepRef::new(d, tray);
        let from = st.book.missions[d].from.clone();
        st.missions.tray = Some(TrayOffer {
            story: here,
            from: from.clone(),
            tab: from.clone(),
            title: "t".into(),
            line: "l".into(),
            items: vec![],
            shown: false,
        });
        let feed = |story| FeedView {
            story,
            from: from.clone(),
            text: vec!["x".into()],
            close: "close".into(),
        };
        st.missions.feeds.push_back(feed(Some(here)));
        st.missions.feeds.push_back(feed(None));
        let call = Call {
            story: Some(here),
            from: from.clone(),
            lines: vec!["x".into()],
            rang: 0,
            answered: None,
            line: 0,
            shown: 0,
            full: false,
        };
        st.missions.call = Some(call.clone());
        st.missions.calls.push_back(call);
        st.missions.scenes.push_back((Some(here), "odile".into()));
        st.missions.news_ends.push(here);
        st.missions.news.as_mut().expect("the news was on").story = Some(here);
        st.journal.mails[0].story = Some(here);
        let rig = st.home.unwrap();
        let door = crate::track::Source::Spot {
            place: rig.place(),
            pos: rig.home.door,
            radius: 0,
        };
        let tracked = crate::track::set(&mut st, door);
        assert!(tracked.is_ok(), "{tracked:?}");
        st.track.as_mut().unwrap().source = crate::track::Source::Story(here);
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        book.missions[d].steps.remove(tray);

        let (_, s, lost) = read(&path, &book).expect("the run reads");
        assert_eq!(lost, ["a step of doctor (tray)"], "said once");
        let run = &s.missions.runs[d];
        assert_eq!(run.at, book.missions[d].steps.len(), "at its end");
        assert!(
            matches!(run.state, RunState::Done { .. }),
            "{:?}",
            run.state
        );
        assert_eq!(s.missions.tray, None);
        assert_eq!(s.missions.feeds.len(), 1, "the console's feed stays");
        assert_eq!(s.missions.feeds[0].story, None);
        assert_eq!(s.missions.call, None);
        assert!(s.missions.calls.is_empty());
        assert!(s.missions.scenes.is_empty());
        assert!(s.missions.news_ends.is_empty());
        assert!(s.missions.news.is_none());
        assert_eq!(
            s.journal.mails[0].story, None,
            "the mail stays, its link goes"
        );
        assert_eq!(s.journal.mails.len(), st.journal.mails.len());
        assert!(s.track.is_none());
    }

    /// A run saved in another city of the world, its home's story set aside: a step
    /// gone from the book is dropped from what the set-aside story held, as at home.
    #[test]
    fn a_step_gone_while_away_is_dropped_from_the_home_story() {
        let dir = Dir::new("step-away");
        let (mut st, d, tray) = at_the_tray();
        let here = StepRef::new(d, tray);
        let from = st.book.missions[d].from.clone();
        let call = Call {
            story: Some(here),
            from: from.clone(),
            lines: vec!["x".into()],
            rang: 0,
            answered: None,
            line: 0,
            shown: 0,
            full: false,
        };
        st.missions.call = Some(call.clone());
        st.missions.calls.push_back(call);
        st.journal.mails[0].story = Some(here);
        crate::outside::ensure(&mut st);
        let other = st.outside.as_ref().unwrap().others()[0].0;
        crate::outside::enter(&mut st, other).expect("into another city");
        assert!(st.away.is_some());
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        book.missions[d].steps.remove(tray);

        let (_, s, lost) = read(&path, &book).expect("the run reads");
        assert_eq!(lost, ["a step of doctor (tray)"]);
        let away = s.away.as_ref().expect("home set aside");
        assert_eq!(away.missions.call, None);
        assert!(away.missions.calls.is_empty());
        assert_eq!(away.journal.mails[0].story, None);
        let run = &away.missions.runs[d];
        assert!(
            matches!(run.state, RunState::Done { .. }),
            "{:?}",
            run.state
        );
    }

    /// A step the player already finished, gone from a later book: dropped from what
    /// ended, its read mail's link with it, and nothing said, as nothing of theirs goes.
    #[test]
    fn an_ended_step_gone_from_the_book_goes_unsaid() {
        let dir = Dir::new("ended-gone");
        let (mut st, d, tray) = at_the_tray();
        let (mail, gone) = st
            .journal
            .mails
            .iter()
            .enumerate()
            .find_map(|(i, m)| {
                m.story
                    .filter(|r| r.mission == d && r.step < tray)
                    .map(|r| (i, r.step))
            })
            .expect("a mail an ended doctor step sent");
        // Reading it ended its step.
        st.journal.mails[mail].read = true;
        let path = write(&dir.0, &st).unwrap();
        let mut book = st.book.clone();
        book.missions[d].steps.remove(gone);

        let (_, s, lost) = read(&path, &book).expect("the run reads");
        assert!(lost.is_empty(), "nothing the player had is lost: {lost:?}");
        let run = &s.missions.runs[d];
        assert_eq!(
            book.missions[d].steps[run.at].id, "tray",
            "still at the tray"
        );
        assert_eq!(run.ended.len(), tray - 1, "the others still ended");
        assert_eq!(
            s.journal.mails[mail].story, None,
            "the mail stays, its link goes"
        );
        // The step under way gone too: that is said, once.
        let mut book = book.clone();
        let at = book.missions[d].steps.iter().position(|s| s.id == "tray");
        book.missions[d].steps.remove(at.unwrap());
        let (_, _, lost) = read(&path, &book).expect("the run reads");
        assert_eq!(lost, ["a step of doctor (tray)"]);
    }

    #[test]
    fn an_item_gone_from_the_content_is_dropped_and_said() {
        let dir = Dir::new("item");
        let mut st = running(seren_has_written());
        let gone = ItemId::new("old_knife");
        assert!(st.content.item(&gone).is_none());
        st.character
            .inventory
            .push(game::Stack::new(gone.clone(), 1));
        let path = write(&dir.0, &st).unwrap();
        let mut b = AppState::new();
        let (_, s, lost) = read(&path, &b.book).unwrap();
        restore(&mut b, s);
        after_restore(&mut b, lost);
        assert_eq!(b.character.count(&gone), 0, "the item is gone");
        let said: Vec<_> = b
            .log
            .iter()
            .filter(|e| e.text.starts_with("lost in the update · "))
            .collect();
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].text, "lost in the update · an item (old_knife)");
        assert_eq!(said[0].kind, crate::messages::Kind::Warn);
        b.deck_tab = crate::screens::deck::Tab::Kit;
        let deck = crate::screens::deck::Deck::new(b.tick).restore(&b);
        let _ = crate::screens::shots::draw_stack(&b, &[&deck], (120, 40));
    }

    #[test]
    fn a_piece_gone_from_the_tray_or_the_tab_is_dropped_and_said() {
        let dir = Dir::new("tray");
        let (mut st, d, tray) = at_the_tray();
        let from = st.book.missions[d].from.clone();
        let kept = ItemId::new("neural_port");
        assert!(st.content.item(&kept).is_some());
        st.missions.tray = Some(TrayOffer {
            story: StepRef::new(d, tray),
            from: from.clone(),
            tab: from,
            title: "t".into(),
            line: "l".into(),
            items: vec![kept.clone(), ItemId::new("old_chip")],
            shown: false,
        });
        st.missions.tab = Some(crate::missions::Tab {
            doc: sim::PersonId(0),
            item: ItemId::new("old_port"),
            payer: "x".into(),
            spent: false,
        });
        let path = write(&dir.0, &st).unwrap();
        let mut b = AppState::new();
        let (_, s, lost) = read(&path, &b.book).unwrap();
        restore(&mut b, s);
        after_restore(&mut b, lost);
        assert_eq!(
            b.missions.tray.as_ref().unwrap().items,
            [kept],
            "the known piece stays"
        );
        assert_eq!(b.missions.tab, None);
        for id in ["old_chip", "old_port"] {
            let line = format!("lost in the update · an item ({id})");
            assert!(b.log.iter().any(|e| e.text == line), "{line}");
        }
        // A tray left with nothing goes.
        b.missions.tray.as_mut().unwrap().items = vec![ItemId::new("old_chip")];
        assert_eq!(
            b.missions.forget_unknown(&b.content),
            ["an item (old_chip)"]
        );
        assert_eq!(b.missions.tray, None);
    }

    #[test]
    fn a_step_ref_needs_a_book_in_scope() {
        let book = Book::load().unwrap();
        let r = StepRef::new(1, 2);
        let e = rmp_serde::to_vec_named(&r).expect_err("no book, no ids");
        assert!(e.to_string().contains("no book in scope"), "{e}");
        let bytes = with_book(&book, || rmp_serde::to_vec_named(&r)).unwrap();
        assert!(
            rmp_serde::from_slice::<StepRef>(&bytes).is_err(),
            "nor read back without one"
        );
        let back: StepRef = with_book(&book, || rmp_serde::from_slice(&bytes)).unwrap();
        assert_eq!(back, r);
        assert!(
            rmp_serde::to_vec_named(&r).is_err(),
            "the scope ends with the call"
        );
    }

    /// The runs the first format wrote, calm, busy and out on the open Net, read by this
    /// build, put in place and played: back on the map as a load puts you
    /// (`MapScreen::resumed`), a few frames of the game's own loop, drawn. No fixture is
    /// ever rewritten.
    #[test]
    fn the_first_saved_run_still_opens() {
        use crate::app::Runner;
        use crate::screens::explore::MapScreen;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let fixtures: [(&str, &[u8]); 3] = [
            ("calm", include_bytes!("fixtures/run-v1.run")),
            ("busy", include_bytes!("fixtures/run-v1-busy.run")),
            ("net", include_bytes!("fixtures/run-v1-net.run")),
        ];
        for (name, bytes) in fixtures {
            let dir = Dir::new("v1");
            let path = dir.0.join(format!("{name}.run"));
            std::fs::write(&path, bytes).unwrap();
            let mut st = AppState::new();
            let (h, s, lost) =
                read(&path, &st.book).unwrap_or_else(|e| panic!("the {name} run reads: {e:#}"));
            assert_eq!(h.seed, 8, "{name}");
            assert!(lost.is_empty(), "{name}: {lost:?}");
            restore(&mut st, s);
            after_restore(&mut st, lost);
            assert!(
                st.city.is_some() && st.player.is_some() && st.world.is_some(),
                "{name}"
            );
            assert!(st.missions.begun, "{name}");
            if name == "busy" {
                let p = st.player.as_ref().unwrap();
                assert!(
                    matches!(p.place, citygen::Place::Construct(_)) && p.body.is_some(),
                    "jacked into the construct"
                );
                assert!(st.net.engaged.is_some() && st.net.run.is_some(), "on turns");
                assert!(!st.bots.is_empty() && !st.foes.is_empty() && !st.live.is_empty());
                assert!(st.missions.call.is_some() && st.track.is_some());
                // Written before the open Net: its echo named the construct alone.
                assert!(matches!(
                    st.net.echo.as_ref().map(|e| e.scene),
                    Some(crate::net::Scene::Inside(_))
                ));
                assert!(st.net.holds.is_empty() && st.foes.iter().all(|f| f.ice.is_none()));
            }
            if name == "net" {
                net_run_came_back(&st);
            }
            st.show_fps = false;
            let map = MapScreen::resumed(&mut st);
            let mut runner = Runner::new(Box::new(map), st);
            let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
            for _ in 0..12 {
                assert!(runner.frame(&mut term, &[]).unwrap(), "{name} plays");
            }
        }
    }

    /// The morning after in the small seed-8 city, Seren's word come: the run the
    /// fixture holds.
    fn small_morning_after() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed: 8,
                size: citygen::SizePreset::Small,
                // The district count that gives seed 8 a home at this size.
                districts: 11,
                ..Default::default()
            })
            .unwrap(),
        );
        let home = st.home.expect("a home in the small city").home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        crate::jobs::first_favour(&mut st);
        crate::missions::tests::to_seren_has_written(&mut st);
        st
    }

    /// Writes `fixtures/run-v1.run`, once, when the first format is settled. Never
    /// rewritten to make `the_first_saved_run_still_opens` pass: a field added later
    /// takes `#[serde(default)]`.
    #[test]
    #[ignore = "writes the fixture into the source tree"]
    fn write_run_fixture() {
        write_fixture(running(small_morning_after()), "run-v1.run");
    }

    /// Writes `fixtures/run-v1-busy.run`, once, as `write_run_fixture` does: never
    /// rewritten to make a test pass.
    #[test]
    #[ignore = "writes the fixture into the source tree"]
    fn write_busy_run_fixture() {
        let st = running(busy_run());
        let to = write_fixture(st, "run-v1-busy.run");
        for (part, held) in parts_held(&body_of(&to)) {
            eprintln!("{part}: {}", if held { "held" } else { "EMPTY" });
        }
    }

    /// `st` written to `fixtures/<name>`; the path.
    fn write_fixture(st: AppState, name: &str) -> std::path::PathBuf {
        let dir = Dir::new("fixture");
        let path = write(&dir.0, &st).unwrap();
        let to = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/runs/fixtures")
            .join(name);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(&path, &to).unwrap();
        eprintln!(
            "{}: {} bytes",
            to.display(),
            std::fs::metadata(&to).unwrap().len()
        );
        to
    }

    /// Writes `fixtures/run-v1-net.run`, once, as `write_run_fixture` does: never
    /// rewritten to make a test pass.
    #[test]
    #[ignore = "writes the fixture into the source tree"]
    fn write_net_run_fixture() {
        let st = running(net_run());
        let to = write_fixture(st, "run-v1-net.run");
        for (part, held) in net_parts_held(&body_of(&to)) {
            eprintln!("{part}: {}", if held { "held" } else { "EMPTY" });
        }
    }

    /// The entry at a dotted `path` of map keys in `body`.
    fn value_at<'a>(body: &'a Value, path: &str) -> Option<&'a Value> {
        path.split('.').try_fold(body, |v, key| {
            v.as_map()?
                .iter()
                .find(|(k, _)| k.as_str() == Some(key))
                .map(|(_, v)| v)
        })
    }

    /// Whether a part holds something: not missing, nil or empty.
    fn filled(v: Option<&Value>) -> bool {
        match v {
            None | Some(Value::Nil) => false,
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Map(m)) => !m.is_empty(),
            Some(_) => true,
        }
    }

    /// The parts of a run the calm fixture leaves empty, by their path in its body, and
    /// whether `body` holds something in each (a chrome copy: one with a piece fitted).
    fn parts_held(body: &Value) -> Vec<(String, bool)> {
        let at = |path: &str| value_at(body, path);
        let mut out: Vec<(String, bool)> = BUSY_PARTS
            .iter()
            .map(|p| (p.to_string(), filled(at(p))))
            .collect();
        let fitted = at("character.chrome_copies")
            .and_then(Value::as_map)
            .is_some_and(|m| {
                m.iter().any(|(_, copy)| {
                    copy.as_map()
                        .and_then(|c| c.iter().find(|(k, _)| k.as_str() == Some("fitted")))
                        .and_then(|(_, f)| f.as_array())
                        .is_some_and(|f| f.iter().any(|s| !s.is_nil()))
                })
            });
        out.push(("character.chrome_copies (a piece fitted)".into(), fitted));
        out
    }

    /// The parts `parts_held` looks at, past the chrome copies.
    const BUSY_PARTS: &[&str] = &[
        "fx",
        "foes",
        "bots",
        "live",
        "constructs.made",
        "net.ice",
        "net.traces",
        "net.hacks",
        "net.run",
        "net.echo",
        "net.engaged",
        "backdoors.list",
        "backdoors.route",
        "backdoors.trace",
        "backdoors.logins",
        "helix.hosts",
        "jobs.fixers",
        "missions.feeds",
        "missions.call",
        "missions.calls",
        "missions.visitors",
        "missions.scenes",
        "missions.notes",
        "missions.tray",
        "missions.tab",
        "track",
        "mail_pill",
        // Not `world.dates`: no test can strike one (see `busy_run`).
        "world.displaced",
    ];

    /// The run `fixtures/run-v1-busy.run` holds: the small morning after with every part
    /// the calm fixture leaves empty made to hold something, as far as a test can set it
    /// up. A walker sidestepped you (no date is struck on the strip: `sim`'s dates are
    /// its own, and a week of this city's world strikes none); the rig's HELIX host
    /// made and a fixer's offer rolled; the
    /// Drone cradle in, its drone out, and a sticker on the deck; a goon hunting the
    /// body, a satchel live on its fuse and a mark in flight beside it; a track on the
    /// tower; a backdoor in it, a route through it and the trace walking home; Mezz
    /// mid-line with a call behind, a feed and a scene queued, the doctor's tray held
    /// out and a piece on her tab, a visitor and a note; the mail pill up. Then jacked
    /// into the tower's construct and on turns there: its ICE, a trace, a hack, a crack
    /// in flight and its last roll.
    fn busy_run() -> AppState {
        use crate::missions::{Call, FeedView, Tab, TrayOffer};
        use crate::render::map::TileMap;
        use citygen::{Place, Pos, StratumKind};
        use game::{HackTarget, ItemId, Op, Site, Target};
        let mut st = small_morning_after();
        let city = st.city.clone().unwrap();

        // A walker on the street sidestepped, the player taking their tile.
        {
            let world = st.world.as_mut().unwrap();
            let (who, pos) = world
                .people()
                .iter()
                .find_map(|p| match p.whereabouts {
                    sim::Whereabouts::At { pos, .. } => Some((p.id, pos)),
                    _ => None,
                })
                .expect("someone on the street");
            let from = pos
                .neighbors4()
                .find(|n| city.walkable(StratumKind::Street, *n))
                .unwrap();
            assert!(world.swap(who, from, pos));
        }
        let now = st.net.now(&st);
        let (place, body) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        // Free tiles about the body, nearest first.
        let free: Vec<Pos> = {
            let map = crate::render::map::MapRef::for_place(
                &city,
                &st.interiors,
                &st.constructs,
                place,
                st.minute_of_day(),
            )
            .expect("the body's place is drawn");
            let mut v: Vec<Pos> = (-5..=5)
                .flat_map(|dy| (-5..=5).map(move |dx| Pos::new(body.x + dx, body.y + dy)))
                .filter(|p| *p != body && map.walkable(*p))
                .collect();
            v.sort_by_key(|p| (p.chebyshev(body), p.y, p.x));
            v
        };
        assert!(free.len() >= 8, "room about the body: {free:?}");

        // HELIX, a fixer, the mail pill.
        crate::helix::hosts::ensure_host(&mut st, crate::helix::shell::HostId::Rig);
        let fixers: Vec<sim::PersonId> = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| p.profession == "fixer" && st.jobs.taken(p.id).is_none())
            .map(|p| p.id)
            .collect();
        let offered = fixers
            .into_iter()
            .find_map(|f| crate::jobs::offer(&mut st, f));
        assert!(offered.is_some(), "a fixer's offer");
        crate::mail_mark::testutil::land(&mut st, "Mezz", "you up?");
        crate::mail_mark::notice(&mut st);
        assert!(st.mail_pill.is_some());

        // The Drone cradle in with its drone docked, and out; a sticker on the deck.
        let cradle = ItemId::new("drone_cradle");
        let mut copy = st.character.new_copy(&st.content, &cradle);
        let socket = st.content.item(&cradle).unwrap().sockets[0];
        let drone = st
            .content
            .items
            .iter()
            .find(|i| i.bot.as_ref().is_some_and(|b| b.docks == socket))
            .unwrap()
            .id
            .clone();
        let piece = st.character.new_copy(&st.content, &drone);
        copy.fitted[0] = Some(game::Stack {
            item: drone,
            qty: 1,
            copy: Some(piece),
        });
        st.character.install(Site::Back, cradle, Some(copy));
        crate::bots::sync(&mut st);
        assert_eq!(st.bots.len(), 1, "the drone is out");
        let sticker = st
            .content
            .items
            .iter()
            .find(|i| i.kind == game::ItemKind::Sticker)
            .unwrap()
            .id
            .clone();
        if let Some(deck) = st.character.worn.get_mut(&game::Slot::Deck) {
            deck.stickers.push(game::Stuck {
                sticker,
                x: 2,
                y: 1,
                turn: game::Turn::Quarter,
            });
        }

        // About the body: a goon hunting it, a satchel live, a mark in flight.
        let mut goon = crate::foes::Foe::goon(place, free[4]);
        goon.mode = crate::foes::FoeMode::Hunting {
            seen: body,
            lost: 0,
        };
        st.foes.push(goon);
        let satchel = st.content.item(&ItemId::new("satchel")).unwrap().clone();
        let Some(game::Use::Throw { blast, .. }) = satchel.on_use else {
            panic!("the satchel is thrown")
        };
        st.live.push(crate::blast::Live {
            place,
            pos: free[6],
            blast,
            fx: satchel.fx.clone(),
            name: satchel.name.clone(),
            turns: 1,
            lands: st.tick,
            wakes: Some(body),
        });
        st.fx.push(crate::app::FxInstance {
            fx: game::Fx::Mark {
                colour: crate::net::MARK_SLIP,
                ticks: 600,
            },
            place,
            origin: body,
            radius: 0,
            started: st.tick,
            revealed: Vec::new(),
            targets: vec![free[0]],
            label: String::new(),
            hit: false,
            tint: None,
            arc: None,
        });

        // The tower: tracked, a backdoor in it, a route through it, the trace walking.
        let tower = city
            .buildings
            .iter()
            .filter(|b| crate::constructs::city_gate(&city, b.id).is_some())
            .find(|b| {
                citygen::construct::generate_construct(&city, b.id)
                    .is_some_and(|k| !k.guards.is_empty() && !k.nodes.is_empty())
            })
            .expect("a construct with ICE and a node")
            .id;
        let tracked = crate::track::set(
            &mut st,
            crate::track::Source::Listing(crate::track::ListingId::Building(tower)),
        );
        assert!(tracked.is_ok(), "{tracked:?}");
        st.backdoors.plant_net(tower, 2, city.seed, now);
        st.backdoors.start_route(Some(tower));
        st.backdoors.start_trace(now);
        st.backdoors.logins.push((now, tower, "root".into()));

        // The story: Mezz mid-line, a call behind, a feed and a scene queued, the
        // doctor's tray and tab, a visitor, a note.
        let book = st.book.clone();
        let m = book
            .missions
            .iter()
            .position(|m| m.id == "morning_after")
            .unwrap();
        let here = StepRef::new(m, st.missions.runs[m].at);
        let d = mission_at(&book, "doctor");
        let doc_role = book.missions[d].from.clone();
        let clock = st.clock_tick();
        let call = Call {
            story: Some(here),
            from: "mezz".into(),
            lines: vec!["You up, choom?".into(), "Don't open it.".into()],
            rang: clock,
            answered: Some(clock),
            line: 1,
            shown: clock,
            full: false,
        };
        st.missions.calls.push_back(Call {
            story: None,
            answered: None,
            line: 0,
            ..call.clone()
        });
        st.missions.call = Some(call);
        st.missions.feeds.push_back(FeedView {
            story: Some(here),
            from: "mezz".into(),
            text: vec!["Word on the street.".into()],
            close: "close".into(),
        });
        st.missions.scenes.push_back((Some(here), "waking".into()));
        st.missions.tray = Some(TrayOffer {
            story: StepRef::new(d, step_at(&book, d, "tray")),
            from: doc_role.clone(),
            tab: doc_role.clone(),
            title: "her tray".into(),
            line: "One piece.".into(),
            items: vec![ItemId::new("neural_port")],
            shown: true,
        });
        st.missions.tab = Some(Tab {
            doc: st
                .missions
                .cast
                .get(&doc_role)
                .copied()
                .unwrap_or(sim::PersonId(0)),
            item: ItemId::new("neural_port"),
            payer: doc_role.clone(),
            spent: false,
        });
        crate::missions::visitors::arrive(&mut st, &doc_role, (place, free[2]));
        st.missions
            .notes
            .push((place, free[3], "Someone was here.".into()));

        // Jacked in from where the body stands, into the tower's construct, and on
        // turns there.
        let gate = crate::constructs::city_gate(&city, tower).unwrap();
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((place, body));
            p.place = Place::Stratum(StratumKind::Net);
            p.pos = gate;
        }
        crate::constructs::enter_construct(&mut st, tower).unwrap();
        let k = st.constructs.get(tower).unwrap().clone();
        let (node, _) = k.nodes[0];
        let kind = city.net.node(node).kind;
        st.net.trace(tower).heat = 18;
        st.net.land(
            tower,
            HackTarget::Node(node),
            Target::Node(kind),
            Op::Crack,
            now,
        );
        let crack = game::Run {
            program: ItemId::new("crack"),
            target: HackTarget::Guard {
                building: tower,
                index: 0,
            },
            done: 1,
            work: 3,
        };
        st.net.run = Some(crack.clone());
        st.net.echo = Some(crate::net::RollEcho {
            at: st.tick,
            roll: crate::net::Roll::Hit,
            // Written as `building: tower` by the build that wrote the fixture.
            scene: crate::net::Scene::Inside(tower),
            run: crack,
        });
        crate::net::engage(&mut st, Place::Construct(tower), true);
        assert!(st.net.engaged.is_some() && !st.net.ice.is_empty());
        st
    }

    #[test]
    fn the_busy_fixture_holds_what_the_calm_one_leaves_empty() {
        let dir = Dir::new("busy-parts");
        let path = dir.0.join("busy.run");
        std::fs::write(&path, include_bytes!("fixtures/run-v1-busy.run")).unwrap();
        let empty: Vec<String> = parts_held(&body_of(&path))
            .into_iter()
            .filter(|(_, held)| !held)
            .map(|(part, _)| part)
            .collect();
        assert!(empty.is_empty(), "the fixture leaves empty: {empty:?}");
    }

    /// The open Net's state the calm and busy fixtures were written before, by its path
    /// in a body. Not `net.decoy`: a decoy only lasts inside its own construct, where the
    /// open Net's trail and a street camera's feed are gone (its round trip is
    /// `the_open_nets_state_comes_back_whole`'s).
    const NET_PARTS: &[&str] = &[
        "net.district_traces",
        "net.holds",
        "net.feed",
        "net.hijacks",
        "net.lights_out",
        "net.eyes_minute",
        "net.echo.scene",
        "net.run",
        "net.grid.trail",
        "net.grid.rings",
        "net.grid.alarmed",
        "net.grid.last",
    ];

    /// `NET_PARTS` and whether `body` holds something in each, then the sentinels posted,
    /// a camera's hold with its loop spent, and grid ICE among the foes: a sentinel and
    /// a tracer.
    fn net_parts_held(body: &Value) -> Vec<(String, bool)> {
        let at = |path: &str| value_at(body, path);
        let mut out: Vec<(String, bool)> = NET_PARTS
            .iter()
            .map(|p| (p.to_string(), filled(at(p))))
            .collect();
        out.push((
            "net.grid.posted".into(),
            at("net.grid.posted").and_then(Value::as_bool) == Some(true),
        ));
        let looped = at("net.holds")
            .and_then(Value::as_array)
            .is_some_and(|holds| {
                holds.iter().any(|h| {
                    value_at(h, "used")
                        .and_then(Value::as_array)
                        .is_some_and(|u| u.iter().any(|v| v.as_str() == Some("Loop")))
                })
            });
        out.push(("net.holds (a camera looped)".into(), looped));
        let roles: Vec<String> = at("foes")
            .and_then(Value::as_array)
            .map(|foes| {
                foes.iter()
                    .filter_map(|f| value_at(f, "ice.role")?.as_map()?.first())
                    .filter_map(|(k, _)| k.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        for role in ["Sentinel", "Tracer"] {
            out.push((
                format!("foes (a grid {role})"),
                roles.iter().any(|r| r == role),
            ));
        }
        out
    }

    /// The run `fixtures/run-v1-net.run` holds: the small morning after, jacked in where
    /// you stood and out on the open Net beside a building's street camera, the slice's
    /// work done round it. The camera held, looped and its feed open; the building's ad
    /// board held and hijacked; a street terminal of its district held; the district's
    /// lamp grid held and its lamps cut (by day, so the street shows nothing yet); the
    /// district's trace warmed to its alarm, its sentinels out and woken and a tracer on
    /// your trail; a sweep's ring out of its hub (a dev switch rings them); a crack in
    /// flight on a node beside you and its last roll.
    fn net_run() -> AppState {
        use crate::holds::{self, Line, Verb};
        use crate::owners::Owner;
        use citygen::{NodeKind, Place, Pos, StratumKind};
        use game::HackTarget;
        let mut st = small_morning_after();
        let city = st.city.clone().unwrap();
        let net = Place::Stratum(StratumKind::Net);
        let of_district = |d: citygen::DistrictId, kind: NodeKind| {
            city.net
                .nodes
                .iter()
                .find(|n| {
                    n.kind == kind
                        && crate::owners::owner_of(&city, n.id) == Some(Owner::District(d))
                })
                .map(|n| n.id)
        };
        // A street camera whose building has a street ad board, whose district has a
        // lamp grid, a street terminal and a hub, with a lane beside its wall.
        let (cam, b, d, ad, lamp, term, hub, pos) = crate::cameras::street_cameras(&city)
            .find_map(|cam| {
                let b = cam.building;
                let d = city.buildings[b.index()].district;
                let ad = city.net.nodes.iter().find(|n| {
                    n.building == Some(b)
                        && n.kind == NodeKind::AdBoard
                        && n.anchor.0 == StratumKind::Street
                })?;
                let hub = crate::grid::hubs(&city)
                    .into_iter()
                    .find(|(x, _)| *x == d)?
                    .1;
                let cyber = city.net.node(cam.node).cyber;
                let pos = cyber
                    .neighbors8()
                    .find(|p| city.walkable(StratumKind::Net, *p))?;
                Some((
                    cam.node,
                    b,
                    d,
                    ad.id,
                    of_district(d, NodeKind::LampGrid)?,
                    of_district(d, NodeKind::Terminal)?,
                    hub,
                    pos,
                ))
            })
            .expect("a street camera with an ad board, a lamp grid and a terminal near");

        // Jacked in from where the body stands, out on the open Net by the camera's wall,
        // the trail you came by behind you.
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((p.place, p.pos));
            p.place = net;
            p.pos = pos;
        }
        let mut trail: Vec<Pos> = pos
            .neighbors4()
            .filter(|p| city.walkable(StratumKind::Net, *p))
            .take(2)
            .collect();
        trail.push(pos);
        st.net.grid.trail = trail
            .into_iter()
            .map(|p| (p, crate::grid::by_a_hub(&city, p)))
            .collect();
        st.net.grid.last = Some((net, pos));
        let clock = st.clock_tick();
        let now = st.net.now(&st);

        // The camera held, looped, its feed open.
        holds::take(&mut st, cam, Owner::Building(b), now);
        assert!(holds::use_verb(&mut st, Verb::Loop.key()).is_ok());
        assert!(holds::use_verb(&mut st, Verb::Tap.key()).is_ok());
        assert_eq!(st.net.feed, Some(cam));
        // The ad board held and its building's ads saying your tag.
        holds::take(&mut st, ad, Owner::Building(b), now);
        holds::hijack(&mut st, ad, Line::Tag);
        assert!(!st.net.hijacks.is_empty());
        // A street terminal held: a user's seat on the district's host.
        holds::take(&mut st, term, Owner::District(d), now);
        // The district's lamp grid held and cut.
        holds::take(&mut st, lamp, Owner::District(d), now);
        holds::spend(&mut st, lamp, Verb::LightsOut);
        st.net.lights_out.insert(d, now + holds::LIGHTS_OUT_MINUTES);
        holds::relight(&mut st);
        st.net.eyes_minute = Some(now);
        st.net.line_next = clock + 90;

        // The district's trace up to its alarm: the sentinels out and woken, a tracer
        // off its hub on your trail. A sweep's ring out of the hub.
        crate::grid::post_sentinels(&mut st);
        let o = Owner::District(d);
        crate::owners::heat_on(&mut st, o, 30);
        assert_eq!(
            crate::owners::heat_on(&mut st, o, 35),
            crate::owners::Heat::Alarm
        );
        crate::grid::alarm(&mut st, o);
        st.net.grid.rings.push(crate::grid::Ring {
            district: d,
            hub,
            born: clock,
        });
        st.net.grid.ring_tick = clock;

        // A crack in flight on a node beside you, one roll in.
        let target = crate::net::targets_outside(&st, pos)
            .into_iter()
            .map(|(t, _)| t)
            .find(|t| matches!(t, HackTarget::Node(n) if *n != cam))
            .unwrap_or(HackTarget::Node(cam));
        let crack = game::Run {
            program: ItemId::new("crack"),
            target,
            done: 1,
            work: 3,
        };
        st.net.run = Some(crack.clone());
        st.net.echo = Some(crate::net::RollEcho {
            at: st.tick,
            roll: crate::net::Roll::Hit,
            scene: crate::net::Scene::Outside,
            run: crack,
        });
        st.net.beat = clock;
        st
    }

    /// The net fixture as it comes back: on the open Net, the camera still looped and
    /// its feed open, the lamps still cut (the street's dark made again), the district
    /// still on alert with its ICE out.
    fn net_run_came_back(st: &AppState) {
        use citygen::{NodeKind, Place, StratumKind};
        let p = st.player.as_ref().unwrap();
        assert!(
            p.place == Place::Stratum(StratumKind::Net) && p.body.is_some(),
            "out on the open Net"
        );
        let city = st.city.as_ref().unwrap();
        let now = st.net.now(st);
        let cam = st
            .net
            .holds
            .iter()
            .find(|h| city.net.node(h.node).kind == NodeKind::Camera)
            .expect("the camera's hold")
            .node;
        assert!(crate::holds::looped(st, cam, now) && st.net.blinded(cam, now));
        assert_eq!(st.net.feed, Some(cam), "its feed open");
        assert_eq!(
            st.net.holds.len(),
            4,
            "camera, ad board, terminal, lamp grid"
        );
        assert!(!crate::holds::hijacked_ads(st).is_empty());
        let (&d, _) = st.net.lights_out.first_key_value().expect("lights out");
        let street = city.stratum(StratumKind::Street).unwrap();
        assert!(street.dark.contains(&d), "the street's dark is made again");
        let o = crate::owners::Owner::District(d);
        assert!(crate::owners::alert(st, o, now) && st.net.grid.alarmed.contains(&o));
        assert!(st.net.grid.posted && !st.net.grid.trail.is_empty());
        assert!(
            st.foes
                .iter()
                .any(|f| f.ice.as_ref().is_some_and(|i| i.tracer()))
        );
        assert_eq!(
            st.net.echo.as_ref().map(|e| e.scene),
            Some(crate::net::Scene::Outside)
        );
    }

    #[test]
    fn the_net_fixture_holds_the_open_nets_state() {
        let dir = Dir::new("net-parts");
        let path = dir.0.join("net.run");
        std::fs::write(&path, include_bytes!("fixtures/run-v1-net.run")).unwrap();
        let empty: Vec<String> = net_parts_held(&body_of(&path))
            .into_iter()
            .filter(|(_, held)| !held)
            .map(|(part, _)| part)
            .collect();
        assert!(empty.is_empty(), "the fixture leaves empty: {empty:?}");
    }

    /// The open Net's state written and read back whole, the street's dark made again
    /// from the lights out; and the parts no fixture holds (a decoy, a run on grid ICE).
    #[test]
    fn the_open_nets_state_comes_back_whole() {
        use crate::runs::snapshot::snapshot;
        use crate::runs::testutil::{round, same};
        use citygen::StratumKind;
        let dir = Dir::new("open-net");
        let a = running(net_run());
        let path = write(&dir.0, &a).unwrap();
        let mut b = AppState::new();
        let (_, s, lost) = read(&path, &b.book).unwrap();
        assert!(lost.is_empty(), "{lost:?}");
        restore(&mut b, s);
        after_restore(&mut b, lost);
        assert!(
            same(&snapshot(&a), &snapshot(&b)),
            "the run comes back whole"
        );
        assert_eq!(b.net.holds, a.net.holds);
        assert_eq!(b.net.grid, a.net.grid);
        let dark = |st: &AppState| {
            st.city
                .as_ref()
                .unwrap()
                .stratum(StratumKind::Street)
                .unwrap()
                .dark
                .clone()
        };
        assert!(!dark(&a).is_empty());
        assert_eq!(dark(&b), dark(&a), "not saved, but made again");
        net_run_came_back(&b);

        let decoy = crate::holds::Decoy {
            building: citygen::BuildingId(3),
            node: citygen::NodeId(7),
            to: citygen::Pos::new(4, 5),
            beats: crate::holds::DECOY_BEATS,
        };
        assert_eq!(round(&Some(decoy.clone())), Some(decoy));
        let at_ice = game::Run {
            program: ItemId::new("attack"),
            target: game::HackTarget::Foe(2),
            done: 0,
            work: 1,
        };
        assert_eq!(round(&at_ice), at_ice);
    }
}
