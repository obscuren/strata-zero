//! Saving the run: the whole game state, written and read back.
//!
//! One file per run in `runs/` (`file`): a small header the lists read (`RunHeader`,
//! made by `describe`) and the run itself (`snapshot`).

// Nothing outside the tests saves or opens a run until the save points and the
// screens that list runs are wired in; remove this then.
#![allow(dead_code)]

pub mod describe;
pub mod file;
pub mod snapshot;
#[cfg(test)]
pub(crate) mod testutil;

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

/// The runs that flatlined, newest death first.
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
