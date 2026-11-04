//! Helpers for the save tests: comparing two values by what they write, a round trip
//! through MessagePack (both with the shipped book in scope, as a save has its own), a
//! folder of the test's own, a state with a run, and a game on the map that saves.

use crate::app::AppState;
use crate::missions::Book;
use crate::runs::ids::with_book;
use crate::runs::{RunId, RunMeta};
use crate::screens::explore::MapScreen;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The shipped book, read once.
pub(crate) fn book() -> &'static Book {
    static BOOK: OnceLock<Book> = OnceLock::new();
    BOOK.get_or_init(|| Book::load().expect("the missions pack is valid"))
}

/// Whether `a` and `b` write the same. HashMaps write their entries in any order, so
/// the bytes are never compared: both are read back as generic values and every map is
/// sorted by the bytes of its encoded keys, all the way down, before they are compared.
pub(crate) fn same<T: Serialize>(a: &T, b: &T) -> bool {
    with_book(book(), || sorted(value_of(a)) == sorted(value_of(b)))
}

fn value_of<T: Serialize>(t: &T) -> rmpv::Value {
    let bytes = rmp_serde::to_vec_named(t).expect("write");
    rmpv::decode::read_value(&mut bytes.as_slice()).expect("read back as a value")
}

fn bytes_of(v: &rmpv::Value) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, v).expect("encode a key");
    out
}

fn sorted(v: rmpv::Value) -> rmpv::Value {
    use rmpv::Value;
    match v {
        Value::Map(entries) => {
            let mut keyed: Vec<(Vec<u8>, Value, Value)> = entries
                .into_iter()
                .map(|(k, v)| {
                    let k = sorted(k);
                    (bytes_of(&k), k, sorted(v))
                })
                .collect();
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(keyed.into_iter().map(|(_, k, v)| (k, v)).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// `t` written and read back.
pub(crate) fn round<T: Serialize + DeserializeOwned>(t: &T) -> T {
    with_book(book(), || {
        let bytes = rmp_serde::to_vec_named(t).expect("write");
        rmp_serde::from_slice(&bytes).expect("read")
    })
}

/// A folder of its own under the temp dir, removed when the test is done with it.
pub(crate) struct Dir(pub PathBuf);

impl Dir {
    pub(crate) fn new(name: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!(
            "strata-runs-{}-{name}-{:x}",
            std::process::id(),
            RunId::fresh(0).0
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The state with a run of its own, as JACK IN starts one.
pub(crate) fn running(mut st: AppState) -> AppState {
    st.run = Some(RunMeta {
        id: RunId::fresh(st.tick),
        started: st.tick.saturating_sub(30 * 60),
        born: 0,
    });
    st
}

/// A small city, quick to build.
pub(crate) fn small_city() -> citygen::City {
    citygen::Generator::run(citygen::GenParams {
        seed: 3,
        size: citygen::SizePreset::Small,
        districts: 4,
        ..Default::default()
    })
    .unwrap()
}

/// The seed-8 city the story is played in, with a home.
pub(crate) fn story_city() -> citygen::City {
    citygen::Generator::run(citygen::GenParams {
        seed: 8,
        size: citygen::SizePreset::Medium,
        districts: 7,
        ..Default::default()
    })
    .unwrap()
}

/// `city` walked into with a run and `dir` to save it in, as a game stands once JACK
/// IN is done; the story not begun, so nothing but the test asks for a save.
pub(crate) fn saving(dir: &Dir, city: citygen::City) -> (AppState, MapScreen) {
    let mut st = AppState::new();
    st.enter_city(city);
    st.show_fps = false;
    let map = MapScreen::walking(&mut st);
    let mut st = running(st);
    st.saves = Some(dir.0.clone());
    (st, map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::shots::{on_the_open_net, seren_has_written, tracked_city};

    #[test]
    fn same_ignores_the_order_of_a_map() {
        use std::collections::HashMap;
        let a: HashMap<u32, &str> = (0..40).map(|i| (i, "x")).collect();
        let mut b: HashMap<u32, &str> = HashMap::with_capacity(500);
        for i in (0..40).rev() {
            b.insert(i, "x");
        }
        assert!(same(&a, &b));
        b.insert(7, "y");
        assert!(!same(&a, &b));
    }

    #[test]
    fn sets_come_back_the_same_in_any_order() {
        use citygen::{Place, Pos, StratumKind};
        let mut j = crate::credentials::Journal::default();
        for i in 0..12 {
            j.booted
                .insert((Place::Stratum(StratumKind::Street), Pos::new(i * 3, 40 - i)));
        }
        for _ in 0..5 {
            assert!(same(&j, &round(&j)));
        }
        let (st, _) = on_the_open_net();
        let w = st.world.as_ref().unwrap();
        assert!(w.open_buildings().count() >= 2, "several buildings open");
        for _ in 0..5 {
            assert!(same(w, &round(w)));
        }
    }

    #[test]
    fn every_run_part_comes_back() {
        let st = seren_has_written();
        assert!(
            !st.missions.runs.is_empty() && st.missions.begun,
            "a story under way"
        );
        assert!(!st.journal.mails.is_empty(), "mail in the inbox");
        assert!(!st.missions.unsealed.is_empty(), "items unsealed");
        // `helix.hosts` is empty in this state (hosts are made on first connect).
        // Their maps are private: holding something is being unlike a fresh one.
        assert!(
            !same(&st.interiors, &crate::interiors::Interiors::default()),
            "floors entered"
        );
        // No construct has been entered in this state, so `constructs` is empty here.
        let p = st.player.as_ref().unwrap();
        assert!(same(p, &round(p)), "player");
        assert!(same(&st.net, &round(&st.net)), "net");
        assert!(same(&st.backdoors, &round(&st.backdoors)), "backdoors");
        assert!(same(&st.journal, &round(&st.journal)), "journal");
        assert!(same(&st.helix, &round(&st.helix)), "helix");
        assert!(same(&st.jobs, &round(&st.jobs)), "jobs");
        assert!(same(&st.missions, &round(&st.missions)), "missions");
        assert!(same(&st.log, &round(&st.log)), "log");
        assert!(same(&st.interiors, &round(&st.interiors)), "interiors");
        assert!(same(&st.constructs, &round(&st.constructs)), "constructs");
        assert!(same(&st.ground, &round(&st.ground)), "ground");
        assert!(same(&st.character, &round(&st.character)), "character");
    }

    #[test]
    fn the_net_comes_back_jacked_in() {
        let (st, _) = on_the_open_net();
        let p = st.player.as_ref().unwrap();
        assert!(p.body.is_some(), "the setup is jacked in");
        let back = round(p);
        assert_eq!(back.body, p.body);
        assert!(same(p, &back), "player");
        assert!(same(&st.net, &round(&st.net)), "net");
        assert!(same(&st.constructs, &round(&st.constructs)), "constructs");
    }

    #[test]
    fn a_tracked_walk_comes_back() {
        let (mut st, _) = tracked_city();
        let was = st.track.take().expect("tracked");
        let (target, next) = was.walk_for_test();
        let loaded = round(&was);
        assert_ne!(loaded.walk_for_test().0, target, "the walk is not saved");
        st.track = Some(loaded);
        crate::track::refollow(&mut st);
        let now = st.track.as_ref().expect("still tracked after refollow");
        assert_eq!(now.source, was.source);
        assert_eq!(now.label, was.label);
        assert_eq!(now.walk_for_test(), (target, next));
    }
}
