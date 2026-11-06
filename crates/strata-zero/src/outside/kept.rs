//! The cities and towns you've been in but the home city, each kept whole as you left
//! it, and how a save keeps them small: all a city holds but its map, and its map as the
//! cells it has changed from its build. A city or town is the same from its world every
//! time it's built, so on reopening its map is built again (`remake`) and its changes
//! put back. Nothing in play changes a map's cells today (a lights out's `dark` is
//! skipped and made again), so the changes are none, and a kept city costs a save a few
//! KB. See `docs/saving.md`, "The world beyond the city".

use super::{Settlement, Stay};
use crate::app::AppState;
use citygen::{City, Grid, Pos, Size, StratumKind, Tile};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// The side of the square of cells a map's changes are measured and kept by.
const CHUNK: i32 = 8;

/// The cities and towns kept whole, and the record of each map as it was built.
/// Saved: each kept one as all it holds but its map (`StayParts`), its build's
/// fingerprint and its changes; read back, each waits in `saved` until `remake` builds
/// its map again. A copy starts empty.
#[derive(Default)]
pub struct Kept {
    /// Each as you left it (`keep`, `take`).
    pub(super) stays: BTreeMap<Settlement, Stay>,
    /// Each kept one's changes, measured at the first save after it was left (a map
    /// held but not live is never changed).
    measured: BTreeMap<Settlement, OnceCell<Vec<Change>>>,
    /// Each map as it was built, the one you're in too: what a save measures its
    /// changes against. Never saved: made again as the maps are.
    pub(super) built: BTreeMap<Settlement, Built>,
    /// Read from a save, waiting for its map (`remake`).
    saved: BTreeMap<Settlement, Saved>,
}

impl Clone for Kept {
    fn clone(&self) -> Kept {
        Kept::default()
    }
}

impl std::fmt::Debug for Kept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kept")
            .field("stays", &self.stays.keys().collect::<Vec<_>>())
            .field("saved", &self.saved.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Kept {
    /// City or town `s` kept as you leave it.
    pub(super) fn keep(&mut self, s: Settlement, stay: Stay) {
        self.stays.insert(s, stay);
        self.measured.insert(s, OnceCell::new());
    }

    /// City or town `s` taken out to be live again.
    pub(super) fn take(&mut self, s: Settlement) -> Option<Stay> {
        self.measured.remove(&s);
        self.stays.remove(&s)
    }

    /// What a save read and `remake` hasn't built yet, to change (a step the book lacks
    /// dropped from them: `runs::ids::drop_lost`).
    pub(crate) fn saved_mut(&mut self) -> impl Iterator<Item = &mut StayParts> {
        self.saved.values_mut().map(|s| &mut s.stay)
    }
}

/// A kept city or town on disk.
#[derive(Serialize, Deserialize)]
struct Saved {
    stay: StayParts,
    /// Its map's build, as `Built::fingerprint`: the build that remakes it must match.
    built: u64,
    /// Its map's cells changed since it was built.
    #[serde(default)]
    changes: Vec<Change>,
}

/// `Saved`, borrowed to be written.
#[derive(Serialize)]
struct SavedRef<'a> {
    stay: StayRef<'a>,
    built: u64,
    changes: &'a [Change],
}

impl Serialize for Kept {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.stays.len() + self.saved.len()))?;
        for (k, stay) in &self.stays {
            // a map with no record is measured against itself
            let own;
            let built = match self.built.get(k) {
                Some(b) => b,
                None => {
                    own = Built::of(&stay.city);
                    &own
                }
            };
            let measure = || built.changes(&stay.city);
            let fresh;
            let changes = match self.measured.get(k) {
                Some(m) => m.get_or_init(measure),
                None => {
                    fresh = measure();
                    &fresh
                }
            };
            let saved = SavedRef {
                stay: StayRef::of(stay),
                built: built.fingerprint(),
                changes,
            };
            map.serialize_entry(k, &saved)?;
        }
        for (k, saved) in &self.saved {
            map.serialize_entry(k, saved)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Kept {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Kept, D::Error> {
        Ok(Kept {
            saved: BTreeMap::deserialize(d)?,
            ..Kept::default()
        })
    }
}

/// A `Stay` but its map: how a save keeps a city or town held but not live, its map
/// built again from the world.
#[derive(Serialize, Deserialize)]
pub(crate) struct StayParts {
    pub player: Option<crate::walker::Player>,
    pub world: Option<sim::World>,
    pub interiors: crate::interiors::Interiors,
    pub constructs: crate::constructs::Constructs,
    pub net: crate::net::NetState,
    pub backdoors: crate::backdoors::Backdoors,
    pub ground: game::Ground,
    pub foes: Vec<crate::foes::Foe>,
    pub bots: Vec<crate::bots::BotOut>,
    pub live: Vec<crate::blast::Live>,
    pub journal: crate::credentials::Journal,
    pub jobs: crate::jobs::Jobs,
    pub missions: crate::missions::Missions,
    pub track: Option<crate::track::Track>,
    pub helix: crate::helix::Helix,
    pub mail_seen: usize,
    pub mail_pill: Option<crate::mail_mark::MailPill>,
    pub cards: Vec<game::Item>,
    pub card_stacks: Vec<game::Stack>,
}

/// `StayParts`, borrowed from a `Stay` to be written.
#[derive(Serialize)]
struct StayRef<'a> {
    player: &'a Option<crate::walker::Player>,
    world: &'a Option<sim::World>,
    interiors: &'a crate::interiors::Interiors,
    constructs: &'a crate::constructs::Constructs,
    net: &'a crate::net::NetState,
    backdoors: &'a crate::backdoors::Backdoors,
    ground: &'a game::Ground,
    foes: &'a Vec<crate::foes::Foe>,
    bots: &'a Vec<crate::bots::BotOut>,
    live: &'a Vec<crate::blast::Live>,
    journal: &'a crate::credentials::Journal,
    jobs: &'a crate::jobs::Jobs,
    missions: &'a crate::missions::Missions,
    track: &'a Option<crate::track::Track>,
    helix: &'a crate::helix::Helix,
    mail_seen: &'a usize,
    mail_pill: &'a Option<crate::mail_mark::MailPill>,
    cards: &'a Vec<game::Item>,
    card_stacks: &'a Vec<game::Stack>,
}

impl<'a> StayRef<'a> {
    /// Every field but the map named, so one added to `Stay` is weighed here.
    fn of(stay: &'a Stay) -> StayRef<'a> {
        let Stay {
            city: _, // built again from the world
            player,
            world,
            interiors,
            constructs,
            net,
            backdoors,
            ground,
            foes,
            bots,
            live,
            journal,
            jobs,
            missions,
            track,
            helix,
            mail_seen,
            mail_pill,
            cards,
            card_stacks,
        } = stay;
        StayRef {
            player,
            world,
            interiors,
            constructs,
            net,
            backdoors,
            ground,
            foes,
            bots,
            live,
            journal,
            jobs,
            missions,
            track,
            helix,
            mail_seen,
            mail_pill,
            cards,
            card_stacks,
        }
    }
}

impl StayParts {
    /// The stay, its map `city` built again.
    fn with(self, city: City) -> Stay {
        let StayParts {
            player,
            world,
            interiors,
            constructs,
            net,
            backdoors,
            ground,
            foes,
            bots,
            live,
            journal,
            jobs,
            missions,
            track,
            helix,
            mail_seen,
            mail_pill,
            cards,
            card_stacks,
        } = self;
        Stay {
            city,
            player,
            world,
            interiors,
            constructs,
            net,
            backdoors,
            ground,
            foes,
            bots,
            live,
            journal,
            jobs,
            missions,
            track,
            helix,
            mail_seen,
            mail_pill,
            cards,
            card_stacks,
        }
    }
}

/// A map as it was built: each stratum's cells, a hash a chunk (`CHUNK` square, row by
/// row). The hashes are the same on every machine and every run of a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    size: Size,
    strata: Vec<(StratumKind, Vec<u64>)>,
}

/// Cells of a map changed from its build: one chunk's, all of them, row by row (as far
/// as the map goes), on one stratum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    stratum: StratumKind,
    /// The chunk's corner.
    at: Pos,
    tiles: Vec<Tile>,
}

impl Built {
    pub fn of(city: &City) -> Built {
        Built {
            size: city.size,
            strata: city
                .strata
                .iter()
                .map(|s| {
                    (
                        s.kind,
                        chunks(city.size).map(|c| chunk(&s.tiles, c)).collect(),
                    )
                })
                .collect(),
        }
    }

    /// One number for the whole build.
    pub fn fingerprint(&self) -> u64 {
        let mut h = Fold::default();
        (self.size.w, self.size.h).hash(&mut h);
        for (kind, hashes) in &self.strata {
            kind.hash(&mut h);
            hashes.hash(&mut h);
        }
        h.finish()
    }

    /// The chunks of `city` (this map, played on) whose cells aren't as built.
    pub fn changes(&self, city: &City) -> Vec<Change> {
        let mut out = Vec::new();
        for s in &city.strata {
            let was = self
                .strata
                .iter()
                .find(|(k, _)| *k == s.kind)
                .map(|(_, h)| h)
                .filter(|_| self.size == city.size);
            for (i, c) in chunks(city.size).enumerate() {
                if was.and_then(|h| h.get(i)) == Some(&chunk(&s.tiles, c)) {
                    continue;
                }
                out.push(Change {
                    stratum: s.kind,
                    at: c,
                    tiles: cells(city.size, c).map(|p| s.tiles[p]).collect(),
                });
            }
        }
        out
    }
}

/// `changes` put back on `city` as built.
fn apply(city: &mut City, changes: &[Change]) {
    let size = city.size;
    for ch in changes {
        let Some(s) = city.stratum_mut(ch.stratum) else {
            continue;
        };
        for (p, t) in cells(size, ch.at).zip(&ch.tiles) {
            s.tiles.set(p, *t);
        }
    }
}

/// The corners of a map's chunks, row by row.
fn chunks(size: Size) -> impl Iterator<Item = Pos> {
    let (w, h) = (size.w as i32, size.h as i32);
    (0..h)
        .step_by(CHUNK as usize)
        .flat_map(move |y| (0..w).step_by(CHUNK as usize).map(move |x| Pos::new(x, y)))
}

/// The cells of the chunk at `corner`, row by row, as far as the map goes.
fn cells(size: Size, corner: Pos) -> impl Iterator<Item = Pos> {
    let x1 = (corner.x + CHUNK).min(size.w as i32);
    let y1 = (corner.y + CHUNK).min(size.h as i32);
    (corner.y..y1).flat_map(move |y| (corner.x..x1).map(move |x| Pos::new(x, y)))
}

/// The hash of the chunk at `corner`: each cell as two words (its terrain by its place
/// in the list, which only grows at its end, as the saves keep it).
fn chunk(tiles: &Grid<Tile>, corner: Pos) -> u64 {
    let mut h = Fold::default();
    for p in cells(tiles.size(), corner) {
        let t = &tiles[p];
        let feature = t.feature.map_or(0, |f| {
            let mut h = Fold::default();
            f.hash(&mut h);
            h.finish() | 1
        });
        let id = |n: Option<u32>| n.map_or(0, |n| n as u64 + 1);
        h.add(t.terrain as u64 | (t.variant as u64) << 8 | feature << 16);
        h.add(id(t.district.map(|d| d.0)) | id(t.building.map(|b| b.0)) << 32);
    }
    citygen::rng::hash64(h.finish(), 0xC4A2)
}

/// A hasher with no keys and no platform in it: every write folded in by multiply and
/// rotate, so a map hashes the same wherever and whenever it's built.
#[derive(Default)]
struct Fold(u64);

impl Fold {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7C_C1_B7_27_22_0A_95);
    }
}

impl Hasher for Fold {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for part in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..part.len()].copy_from_slice(part);
            self.add(u64::from_le_bytes(word));
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    fn write_u16(&mut self, i: u16) {
        self.add(i as u64);
    }
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    fn write_isize(&mut self, i: isize) {
        self.add(i as u64);
    }
}

/// After a run is reopened: each city or town it kept built again from the world, on
/// threads of their own, its changes put back and all it held with it, its people's
/// places rebuilt (`Stay::reopened`). One this build makes otherwise than the build
/// that saved it, or can't make, is lost, said in the lines returned with whatever
/// `Stay::reopened` drops. The map you're in, if not home's, came whole from the save:
/// what it changes from here on is measured against it.
pub fn remake(st: &mut AppState) -> Vec<String> {
    let jobs = {
        let home = st.away.as_ref().map(|s| &s.city).or(st.city.as_ref());
        let Some(o) = st.outside.as_mut() else {
            return Vec::new();
        };
        // the world goes on without them if it can't be made (`outside::reopened`)
        o.rebuild(home);
        if o.world.is_none() {
            return Vec::new();
        }
        if let (Some(here), Some(city)) = (o.here, st.city.as_ref()) {
            o.kept.built.entry(here).or_insert_with(|| Built::of(city));
        }
        let saved = std::mem::take(&mut o.kept.saved);
        let params = &st.params;
        saved
            .into_iter()
            .map(|(s, k)| (s, super::build_params(o, s, params), k))
            .collect::<Vec<_>>()
    };
    let made = std::thread::scope(|scope| {
        let flight: Vec<_> = jobs
            .into_iter()
            .map(|(s, params, k)| {
                let build = move || {
                    super::stream::build_thread(|| {
                        let city = citygen::Generator::run(params?).ok()?;
                        let built = Built::of(&city);
                        Some((city, built))
                    })
                };
                let name = super::stream::build_name(s);
                let thread = std::thread::Builder::new().name(name);
                (s, k, thread.spawn_scoped(scope, build))
            })
            .collect();
        flight
            .into_iter()
            .map(|(s, k, thread)| {
                let landed = match thread {
                    Ok(t) => t.join().unwrap_or_else(|p| Err(super::stream::said(&*p))),
                    Err(_) => Ok(None),
                };
                (s, k, landed)
            })
            .collect::<Vec<_>>()
    });
    let mut lost = Vec::new();
    for (s, k, landed) in made {
        let Some(name) = st.outside.as_ref().map(|o| o.name(s)) else {
            break;
        };
        let (mut city, built) = match landed {
            Ok(Some((city, built))) if built.fingerprint() == k.built => (city, built),
            Ok(_) => {
                lost.push(format!("what you left in {name}"));
                continue;
            }
            Err(said) => {
                st.warn(format!("building {name} again broke: {said}"));
                lost.push(format!("what you left in {name}"));
                continue;
            }
        };
        apply(&mut city, &k.changes);
        let mut stay = k.stay.with(city);
        lost.extend(stay.reopened(&st.people, &st.content));
        if let Some(o) = st.outside.as_mut() {
            o.kept.built.insert(s, built);
            o.kept.keep(s, stay);
            // measured already: the next save writes them as read
            o.kept.measured.insert(s, OnceCell::from(k.changes));
        }
    }
    lost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outside::stream::tests::settlements;

    /// A town of seed 3's world, as built.
    fn a_town() -> City {
        settlements()
            .iter()
            .find(|(s, _)| s.town.is_some())
            .unwrap()
            .1
            .clone()
    }

    #[test]
    fn a_map_as_built_has_no_changes() {
        let city = a_town();
        let built = Built::of(&city);
        assert!(built.changes(&city).is_empty());
        assert_eq!(Built::of(&city.clone()).fingerprint(), built.fingerprint());
    }

    #[test]
    fn a_changed_cell_is_kept_by_its_chunk_and_put_back() {
        let city = a_town();
        let built = Built::of(&city);
        let mut played = city.clone();
        let p = Pos::new(13, 21);
        let was = *played.tile(StratumKind::Street, p).unwrap();
        let now = Tile::new(citygen::Terrain::Wall);
        assert_ne!(was, now);
        *played.tile_mut(StratumKind::Street, p).unwrap() = now;
        let changes = built.changes(&played);
        assert_eq!(changes.len(), 1, "one chunk");
        assert_eq!(changes[0].at, Pos::new(8, 16));
        assert_eq!(changes[0].tiles.len(), (CHUNK * CHUNK) as usize);
        let mut again = city.clone();
        apply(&mut again, &changes);
        assert!(
            again == played,
            "the build with its changes is the map played"
        );
    }

    #[test]
    fn a_chunk_past_the_maps_edge_keeps_only_its_cells() {
        let city = a_town();
        let built = Built::of(&city);
        let (w, h) = (city.size.w as i32, city.size.h as i32);
        let corner = Pos::new((w - 1) / CHUNK * CHUNK, (h - 1) / CHUNK * CHUNK);
        let mut played = city.clone();
        *played
            .tile_mut(StratumKind::Net, Pos::new(w - 1, h - 1))
            .unwrap() = Tile::new(citygen::Terrain::NetWall);
        let changes = built.changes(&played);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].at, corner);
        assert_eq!(
            changes[0].tiles.len(),
            ((w - corner.x) * (h - corner.y)) as usize
        );
        let mut again = city.clone();
        apply(&mut again, &changes);
        assert!(again == played);
    }

    /// Seed 3's home run, in another city and home again: the city kept.
    fn been_away() -> (AppState, Settlement) {
        let mut st = crate::outside::tests::home_run();
        let (s, _) = crate::outside::tests::beside_another(&st);
        crate::outside::enter(&mut st, s).unwrap();
        let home = st.outside.as_ref().unwrap().home();
        crate::outside::enter(&mut st, home).unwrap();
        (st, s)
    }

    /// `st` saved, its world's fields changed by `edit` as written, and reopened.
    fn reopen_edited(
        st: AppState,
        edit: impl FnOnce(&mut Vec<(rmpv::Value, rmpv::Value)>),
    ) -> AppState {
        let st = crate::runs::testutil::running(st);
        let raw = crate::runs::ids::with_book(&st.book, || {
            rmp_serde::to_vec_named(&crate::runs::snapshot::snapshot(&st))
        })
        .unwrap();
        let mut v = rmpv::decode::read_value(&mut raw.as_slice()).unwrap();
        let rmpv::Value::Map(run) = &mut v else {
            panic!("a run is a map");
        };
        let (_, outside) = run.iter_mut().find(|(k, _)| named(k, "outside")).unwrap();
        let rmpv::Value::Map(outside) = outside else {
            panic!("the world is a map");
        };
        edit(outside);
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, &v).unwrap();
        let mut back = AppState::new();
        let (snap, lost) = crate::runs::ids::read_snapshot(&back.book, &bytes).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        back
    }

    fn named(k: &rmpv::Value, name: &str) -> bool {
        k.as_str() == Some(name)
    }

    #[test]
    fn a_save_from_before_kept_cities_opens() {
        // a run saved before the kept cities were: its world has no `kept`, and the
        // cities you'd been to are made again from the world
        let (st, s) = been_away();
        let back = reopen_edited(st, |outside| {
            let before = outside.len();
            outside.retain(|(k, _)| !named(k, "kept"));
            assert_eq!(outside.len(), before - 1, "the save held them");
        });
        let o = back.outside.as_ref().unwrap();
        assert!(o.visited.contains(&s), "been there");
        assert!(o.held_city(s).is_none(), "none kept");
        let mut back = back;
        crate::outside::enter(&mut back, s).unwrap();
        assert_eq!(back.outside.as_ref().unwrap().here(), s, "made again");
    }

    #[test]
    fn a_city_built_otherwise_is_lost_and_said() {
        // a build whose world makes the kept city otherwise than the one that saved it:
        // what was left there can't sit on the new map, and the log says it's gone
        let (st, s) = been_away();
        let name = st.outside.as_ref().unwrap().name(s);
        let back = reopen_edited(st, |outside| {
            let (_, kept) = outside.iter_mut().find(|(k, _)| named(k, "kept")).unwrap();
            let rmpv::Value::Map(kept) = kept else {
                panic!("kept is a map");
            };
            let rmpv::Value::Map(saved) = &mut kept[0].1 else {
                panic!("a kept city is a map");
            };
            let (_, built) = saved.iter_mut().find(|(k, _)| named(k, "built")).unwrap();
            *built = rmpv::Value::from(1u64);
        });
        assert!(back.outside.as_ref().unwrap().held_city(s).is_none());
        let line = format!("lost in the update · what you left in {name}");
        assert!(back.log.iter().any(|e| e.text == line), "{line}");
    }

    #[test]
    fn a_kept_citys_changed_cells_come_back() {
        // a cell of another city changed while you were in it the second time (a save
        // between measured it as built): kept, saved, reopened, the map made again has
        // it as you left it
        let (mut st, s) = been_away();
        let home = st.outside.as_ref().unwrap().home();
        crate::runs::ids::with_book(&st.book, || {
            rmp_serde::to_vec_named(&crate::runs::snapshot::snapshot(&st))
        })
        .unwrap();
        crate::outside::enter(&mut st, s).unwrap();
        let p = Pos::new(5, 6);
        let wall = Tile::new(citygen::Terrain::Wall);
        let city = st.city.as_mut().unwrap();
        assert_ne!(city.tile(StratumKind::Street, p), Some(&wall));
        *city.tile_mut(StratumKind::Street, p).unwrap() = wall;
        let played = st.city.clone().unwrap();
        crate::outside::enter(&mut st, home).unwrap();
        let back = crate::outside::tests::reopen(st, "kept-changes");
        let kept = back.outside.as_ref().unwrap().held_city(s).unwrap();
        assert_eq!(kept.tile(StratumKind::Street, p), Some(&wall));
        assert!(kept.strata == played.strata, "the map as you left it");
    }

    #[test]
    fn a_different_build_has_a_different_fingerprint() {
        let town = a_town();
        let other = &settlements()
            .iter()
            .find(|(s, c)| s.town.is_some() && c.seed != town.seed)
            .unwrap()
            .1;
        assert_ne!(
            Built::of(&town).fingerprint(),
            Built::of(other).fingerprint()
        );
    }

    /// What kept cities cost a save, in release: seed 3's home run alone, then with three
    /// settlements kept (the three nearest, then the three nearest cities), each saved as
    /// this build saves them (as changes) and as they'd cost whole; the file's size, the
    /// time to write it and to reopen it.
    /// `cargo test -p strata-zero --release --target-dir target/smoke -- --ignored
    /// kept_cities_in_a_save_are_measured --nocapture`.
    #[test]
    #[ignore = "a measurement, in release"]
    fn kept_cities_in_a_save_are_measured() {
        use std::io::Write;
        use std::time::Instant;
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        let median = |mut v: Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        let gz = |bytes: &[u8]| {
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            gz.write_all(bytes).unwrap();
            gz.finish().unwrap().len()
        };
        // the file's size, the first write's ms (the kept cities measured), the write's
        // and the reopen's median ms
        let costs = |st: &AppState| {
            let dir = crate::runs::testutil::Dir::new("kept-measured");
            let (mut write, mut open) = (vec![], vec![]);
            let mut size = 0;
            for _ in 0..5 {
                let t = Instant::now();
                let path = crate::runs::file::write(&dir.0, st).unwrap();
                write.push(ms(t));
                size = std::fs::metadata(&path).unwrap().len() as usize;
                let t = Instant::now();
                let mut back = AppState::new();
                let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
                crate::runs::snapshot::restore(&mut back, snap);
                crate::runs::snapshot::after_restore(&mut back, lost);
                open.push(ms(t));
            }
            (size, write[0], median(write), median(open))
        };
        let alone = crate::runs::testutil::running(crate::outside::tests::home_run());
        let (size0, _, write0, open0) = costs(&alone);
        eprintln!("home alone: {size0} bytes · write {write0:.1} ms · reopen {open0:.1} ms");
        for cities in [false, true] {
            let mut st = crate::outside::tests::home_run();
            let (home, hb) = {
                let o = st.outside.as_ref().unwrap();
                (o.home(), o.block(st.city.as_ref().unwrap()))
            };
            let mut near = st.outside.as_ref().unwrap().others();
            near.retain(|(s, _)| !cities || s.town.is_none());
            near.sort_by_key(|(_, b)| (b.x - hb.x).abs() + (b.y - hb.y).abs());
            for (s, _) in near.into_iter().take(3) {
                crate::outside::enter(&mut st, s).unwrap();
                crate::outside::enter(&mut st, home).unwrap();
            }
            let st = crate::runs::testutil::running(st);
            // as they'd cost whole: each stay, map and all, packed as a save packs it
            let (mut whole, mut whole_ms) = (0, 0.0);
            for (s, stay) in &st.outside.as_ref().unwrap().kept.stays {
                let t = Instant::now();
                let raw = crate::runs::ids::with_book(&st.book, || rmp_serde::to_vec_named(stay))
                    .unwrap();
                whole += gz(&raw);
                whole_ms += ms(t);
                eprintln!("  {s:?}: {:?}", stay.city.size);
            }
            let (size, first, write, open) = costs(&st);
            let what = if cities {
                "three cities"
            } else {
                "the three nearest"
            };
            eprintln!(
                "{what} kept: as changes {size} bytes ({:.2}x) · write {write:.1} ms (the \
                 first {first:.1} ms) · reopen {open:.1} ms; whole {} bytes ({:.2}x) · \
                 write ~{:.1} ms",
                size as f64 / size0 as f64,
                size0 + whole,
                (size0 + whole) as f64 / size0 as f64,
                write0 + whole_ms,
            );
        }
    }
}
