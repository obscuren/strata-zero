//! Saving the run: the whole game state, written and read back.
//!
//! One file per run in `runs/` (`file`): a small header the lists read (`RunHeader`,
//! made by `describe`) and the run itself (`snapshot`), written when the run can take
//! it (`when`).

pub mod describe;
pub mod file;
pub mod ids;
pub mod snapshot;
#[cfg(test)]
pub(crate) mod testutil;
pub mod when;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A run's name on disk: made once, when the run starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub u64);

impl RunId {
    /// An id nobody has used: the wall clock's nanoseconds hashed with the tick.
    pub fn fresh(tick: u64) -> RunId {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        RunId(citygen::rng::hash64(tick, nanos))
    }
}

impl std::fmt::Display for RunId {
    /// Sixteen lowercase hex digits.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// What the game keeps of the run itself: its id, and when it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMeta {
    pub id: RunId,
    /// The frame tick it started on: `st.tick` is saved with the run, so the frames
    /// since are the time played.
    pub started: u64,
    /// The world's minute it started on (`net.now`), for how long it lasted.
    pub born: u64,
}

/// What a run's file says of it before its body is opened: what the lists show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunHeader {
    pub id: RunId,
    pub handle: String,
    /// The profession's display name: `NETRUNNER`.
    pub profession: String,
    pub level: u8,
    /// The city's name.
    pub city: String,
    pub seed: u64,
    pub day: u32,
    /// The minute of the day.
    pub minute: u16,
    /// Where you stand: `Velform Group Plaza 18F · at home`.
    pub place: String,
    /// The title of the mission under way.
    pub mission: Option<String>,
    /// Its objective.
    pub objective: Option<String>,
    pub perma: bool,
    pub credits: u32,
    /// The names of the chrome put in.
    pub chrome: Vec<String>,
    /// How many talents are held.
    pub talents: u32,
    /// Seconds played.
    pub played: u64,
    /// When it was written, in Unix seconds.
    pub saved: u64,
    /// How the run ended, once it has.
    pub death: Option<Death>,
}

/// A flatline, as the HALL OF FAME remembers it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Death {
    pub cause: String,
    pub day: u32,
    pub minute: u16,
    /// When, in Unix seconds.
    pub at: u64,
    /// City minutes from the run's start to its end.
    pub lasted: u64,
}

/// One `.run` file: its header, or why it can't be read.
#[derive(Clone, Debug)]
pub struct RunInfo {
    pub path: PathBuf,
    pub header: Result<RunHeader, String>,
}

/// Where runs are saved: beside `cities/` in the working folder.
pub fn default_dir() -> PathBuf {
    PathBuf::from("runs")
}

/// Every `.run` file in `dir`, by name; one that can't be read carries why. A missing
/// folder lists nothing.
pub fn list(dir: &Path) -> Vec<RunInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<RunInfo> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "run") && p.is_file())
        .map(|path| RunInfo {
            header: file::header(&path).map_err(|e| e.to_string()),
            path,
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// A run whose header says it flatlined.
fn is_dead(r: &RunInfo) -> bool {
    r.header.as_ref().is_ok_and(|h| h.death.is_some())
}

/// The runs still going, newest saved first, then the ones that can't be read.
pub fn living(dir: &Path) -> Vec<RunInfo> {
    let mut out: Vec<RunInfo> = list(dir).into_iter().filter(|r| !is_dead(r)).collect();
    out.sort_by_key(|r| match &r.header {
        Ok(h) => (false, std::cmp::Reverse(h.saved)),
        Err(_) => (true, std::cmp::Reverse(0)),
    });
    out
}

/// The newest run still going whose file reads: what CONTINUE opens.
pub fn newest(dir: &Path) -> Option<(PathBuf, RunHeader)> {
    living(dir)
        .into_iter()
        .find_map(|r| Some((r.path, r.header.ok()?)))
}

/// Puts the run at `path` in place of whatever `st` holds, as it was saved: read against
/// the story's book, restored and made whole, whatever this build lacks of it said in
/// the log. It was last saved when its file says, and the SAVED mark stays down:
/// opening a run isn't saving it. Nothing changes when the file can't be read, and a run
/// that flatlined never opens.
pub fn open(st: &mut crate::app::AppState, path: &Path) -> anyhow::Result<()> {
    let (header, snap, lost) = file::read(path, &st.book)?;
    if header.death.is_some() {
        anyhow::bail!("it flatlined");
    }
    snapshot::restore(st, snap);
    snapshot::after_restore(st, lost);
    let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(header.saved);
    // The frame it was saved on is the frame restored; put just out of the mark's reach.
    st.last_save = Some((st.tick.saturating_sub(when::MARK), at));
    Ok(())
}

/// The runs that flatlined, newest death first.
// The HALL OF FAME lists them once a run can flatline into it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn dead(dir: &Path) -> Vec<RunInfo> {
    let mut out: Vec<RunInfo> = list(dir).into_iter().filter(is_dead).collect();
    out.sort_by_key(|r| {
        std::cmp::Reverse(
            r.header
                .as_ref()
                .ok()
                .and_then(|h| h.death.as_ref())
                .map_or(0, |d| d.at),
        )
    });
    out
}

#[cfg(test)]
mod tests {
    use super::testutil::{Dir, saving, small_city};
    use super::*;
    use crate::app::AppState;

    fn unix(t: std::time::SystemTime) -> u64 {
        t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
    }

    #[test]
    fn opening_a_run_puts_it_in_place_without_the_mark() {
        let dir = Dir::new("open");
        let (mut a, _) = saving(&dir, small_city());
        a.tick = 2_000;
        a.character.handle = "Nyx".into();
        assert!(when::save_now(&mut a));
        let path = dir.0.join(format!("{}.run", a.run.unwrap().id));
        let saved = file::header(&path).unwrap().saved;
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        open(&mut st, &path).unwrap();
        assert_eq!(st.character.handle, "Nyx");
        assert_eq!((st.run, st.tick), (a.run, 2_000));
        assert!(!when::marked(&st), "opening a run is not a save");
        assert_eq!(st.last_save.map(|(_, at)| unix(at)), Some(saved));
        // The next five-minute save is five minutes of play off, give or take the mark.
        let (t, _) = st.last_save.unwrap();
        assert!(st.tick - t <= when::MARK, "{t} against {}", st.tick);
        assert!(!st.save_due);
    }

    #[test]
    fn a_flatlined_run_never_opens() {
        let dir = Dir::new("open-dead");
        let (mut a, _) = saving(&dir, small_city());
        assert!(when::save_now(&mut a));
        let path = dir.0.join(format!("{}.run", a.run.unwrap().id));
        let mut h = file::header(&path).unwrap();
        h.death = Some(Death {
            cause: "shot".into(),
            day: 1,
            minute: 0,
            at: 1,
            lasted: 1,
        });
        file::rewrite_header(&path, &h).unwrap();
        let mut st = AppState::new();
        assert!(open(&mut st, &path).is_err());
        assert!(
            st.run.is_none() && st.city.is_none(),
            "nothing put in place"
        );
    }

    #[test]
    fn continue_is_the_newest_run_that_reads() {
        let dir = Dir::new("newest");
        assert!(newest(&dir.0).is_none());
        std::fs::write(dir.0.join("broken.run"), b"no").unwrap();
        assert!(
            newest(&dir.0).is_none(),
            "an unreadable file isn't a run to go on with"
        );
        let (mut a, _) = saving(&dir, small_city());
        a.character.handle = "Nyx".into();
        assert!(when::save_now(&mut a));
        let (path, h) = newest(&dir.0).expect("the run");
        assert_eq!(h.handle, "Nyx");
        assert_eq!(path, dir.0.join(format!("{}.run", a.run.unwrap().id)));
    }
}
