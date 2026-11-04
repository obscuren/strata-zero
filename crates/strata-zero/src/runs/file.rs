//! The run file: the magic `SZRN`, a little-endian `u16` format version, a
//! little-endian `u32` header length, the header (MessagePack with named fields), then
//! the body: the snapshot, the same, gzipped. Written beside itself and renamed over the
//! old file, so a crash mid-write keeps the last good save.

use super::snapshot::{Snapshot, snapshot};
use super::{RunHeader, describe, ids};
use crate::app::AppState;
use crate::missions::Book;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8; 4] = b"SZRN";
/// 1: the first.
pub const FORMAT_VERSION: u16 = 1;
/// The magic, the version and the header's length.
const PREAMBLE: usize = 10;

/// Why a file can't be listed or opened, as the lists say it.
pub const CANT_OPEN: &str = "this build can't open it";
const NOT_A_RUN: &str = "not a run";
const CUT_SHORT: &str = "cut short";
const TOO_LONG: &str = "too long";
const UNREADABLE: &str = "can't read it";

/// What the header's bytes hold: the header, and the body's length, so a file cut
/// short shows in the list without its body being opened.
#[derive(Serialize, Deserialize)]
struct Head {
    run: RunHeader,
    body: u64,
}

/// Writes the run in `st` to `<dir>/<id>.run`, the folder made if need be; the path.
/// Changes nothing in `st`. A state with no run has nothing to write.
pub fn write(dir: &Path, st: &AppState) -> Result<PathBuf> {
    let Some(run) = st.run else {
        bail!("no run to save");
    };
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    {
        let mut out = std::io::BufWriter::new(&mut gz);
        // The story's positions go by the book's ids.
        ids::with_book(&st.book, || {
            rmp_serde::encode::write_named(&mut out, &snapshot(st))
        })
        .context("writing the run")?;
        out.flush().context("writing the run")?;
    }
    let body = gz.finish().context("packing the run")?;
    let head = Head {
        run: describe::header(st, None),
        body: body.len() as u64,
    };
    std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    let path = dir.join(format!("{}.run", run.id));
    put(&path, &head, &body)?;
    Ok(path)
}

/// The whole file at `path` in place: to `<path>.tmp`, flushed, then renamed over it.
fn put(path: &Path, head: &Head, body: &[u8]) -> Result<()> {
    let header = rmp_serde::to_vec_named(head).context("writing the header")?;
    let tmp = path.with_extension("run.tmp");
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(MAGIC)?;
        f.write_all(&FORMAT_VERSION.to_le_bytes())?;
        f.write_all(&(header.len() as u32).to_le_bytes())?;
        f.write_all(&header)?;
        f.write_all(body)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {}", tmp.display()));
    }
    std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))
}

/// The header's length, once the preamble says this is a run this build reads.
fn preamble(pre: &[u8]) -> Result<usize> {
    if pre.len() < MAGIC.len() || &pre[..MAGIC.len()] != MAGIC {
        bail!(NOT_A_RUN);
    }
    if pre.len() < PREAMBLE {
        bail!(CUT_SHORT);
    }
    match u16::from_le_bytes([pre[4], pre[5]]) {
        0 => bail!(NOT_A_RUN),
        v if v > FORMAT_VERSION => bail!(CANT_OPEN),
        _ => {}
    }
    Ok(u32::from_le_bytes([pre[6], pre[7], pre[8], pre[9]]) as usize)
}

/// The header's bytes read, and checked against the file's length.
fn head(bytes: &[u8], file_len: u64) -> Result<Head> {
    let head: Head = rmp_serde::from_slice(bytes).map_err(|_| anyhow!(CANT_OPEN))?;
    let whole = ((PREAMBLE + bytes.len()) as u64).saturating_add(head.body);
    match file_len.cmp(&whole) {
        std::cmp::Ordering::Less => bail!(CUT_SHORT),
        std::cmp::Ordering::Greater => bail!(TOO_LONG),
        std::cmp::Ordering::Equal => Ok(head),
    }
}

/// A file's header and its body's bytes.
fn split(bytes: &[u8]) -> Result<(Head, &[u8])> {
    let header_len = preamble(&bytes[..bytes.len().min(PREAMBLE)])?;
    let end = PREAMBLE
        .checked_add(header_len)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| anyhow!(CUT_SHORT))?;
    let head = head(&bytes[PREAMBLE..end], bytes.len() as u64)?;
    Ok((head, &bytes[end..]))
}

/// The header of the run at `path`, its body left unread.
pub fn header(path: &Path) -> Result<RunHeader> {
    let mut f = std::fs::File::open(path).map_err(|_| anyhow!(UNREADABLE))?;
    let file_len = f.metadata().map_err(|_| anyhow!(UNREADABLE))?.len();
    let mut pre = Vec::with_capacity(PREAMBLE);
    (&mut f)
        .take(PREAMBLE as u64)
        .read_to_end(&mut pre)
        .map_err(|_| anyhow!(UNREADABLE))?;
    let header_len = preamble(&pre)?;
    if file_len < PREAMBLE as u64 + header_len as u64 {
        bail!(CUT_SHORT);
    }
    let mut bytes = vec![0; header_len];
    f.read_exact(&mut bytes).map_err(|_| anyhow!(UNREADABLE))?;
    Ok(head(&bytes, file_len)?.run)
}

/// The run at `path` read against `book`: its header, its snapshot for
/// `snapshot::restore`, and a line for each mission or step it had that the book lacks
/// (what held them is gone), for `snapshot::after_restore` to say.
pub fn read(path: &Path, book: &Book) -> Result<(RunHeader, Snapshot, Vec<String>)> {
    let bytes = std::fs::read(path).map_err(|_| anyhow!(UNREADABLE))?;
    let (head, body) = split(&bytes)?;
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(body)
        .read_to_end(&mut raw)
        .context("the run is broken")?;
    let (snap, lost) = ids::read_snapshot(book, &raw).context("the run is broken")?;
    Ok((head.run, snap, lost))
}

/// Puts `header` in place of the file's own, its body kept byte for byte.
// The flatline marks a perma death run with it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn rewrite_header(path: &Path, header: &RunHeader) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|_| anyhow!(UNREADABLE))?;
    let (_, body) = split(&bytes)?;
    let head = Head {
        run: header.clone(),
        body: body.len() as u64,
    };
    put(path, &head, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::runs::snapshot::{after_restore, restore, snapshot};
    use crate::runs::testutil::{Dir, book, running, same};
    use crate::runs::{RunId, RunMeta};
    use crate::screens::explore::MapScreen;
    use crate::screens::testutil::buffer_text;
    use std::path::PathBuf;

    /// `path` read back into a new state, made whole.
    fn reopened(path: &std::path::Path) -> AppState {
        let mut st = AppState::new();
        let (_, s, lost) = read(path, &st.book).expect("the run reads back");
        assert!(
            lost.is_empty(),
            "nothing is lost in the same build: {lost:?}"
        );
        restore(&mut st, s);
        after_restore(&mut st, lost);
        st
    }

    /// A small city entered, with a run: enough for a file.
    fn small_run() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed: 3,
                size: citygen::SizePreset::Small,
                districts: 4,
                ..Default::default()
            })
            .unwrap(),
        );
        running(st)
    }

    #[test]
    fn a_run_comes_back_whole() {
        let dir = Dir::new("whole");
        let a = running(crate::screens::shots::seren_has_written());
        let path = write(&dir.0, &a).expect("the run writes");
        assert_eq!(
            path,
            dir.0.join(format!("{}.run", a.run.unwrap().id)),
            "one file, named for the run"
        );
        let size = std::fs::metadata(&path).unwrap().len();
        eprintln!("the morning-after save: {size} bytes");
        assert!(size < 4 * 1024 * 1024, "{size} bytes");
        assert!(
            !dir.0
                .join(format!("{}.run.tmp", a.run.unwrap().id))
                .exists(),
            "the temp file is renamed away"
        );
        let b = reopened(&path);
        assert!(
            same(&snapshot(&a), &snapshot(&b)),
            "the run comes back whole"
        );
        assert_eq!(b.home, a.home, "the home is found again");
        let had = crate::credentials::run_items(&a.content);
        assert!(!had.is_empty(), "the run made keycards");
        assert_eq!(crate::credentials::run_items(&b.content), had);
        // The header says what the lists show.
        let h = header(&path).unwrap();
        assert_eq!(h.id, a.run.unwrap().id);
        assert_eq!(h.handle, a.character.handle);
        assert_eq!(h.profession, "NETRUNNER");
        assert_eq!(h.city, a.city.as_ref().unwrap().name);
        assert_eq!(h.seed, a.city.as_ref().unwrap().seed);
        assert_eq!(h.minute, a.minute_of_day());
        let run = a.run.unwrap();
        assert_eq!(
            h.played,
            (a.tick - run.started) / 30,
            "thirty frames a second"
        );
        assert!(h.place.ends_with(" · at home"), "{}", h.place);
        assert!(h.mission.is_some() && h.objective.is_some());
        assert_eq!((h.mission, h.objective), crate::runs::describe::story(&a));
        assert!(h.death.is_none());
    }

    #[test]
    fn reopens_on_the_floor_it_was_saved_on() {
        let size = (120, 40);
        let dir = Dir::new("floor");
        let (a, _) = crate::screens::shots::beside_the_bed(size);
        let mut a = running(a);
        let path = write(&dir.0, &a).unwrap();
        let mut b = reopened(&path);
        let p = b.player.as_ref().unwrap();
        assert!(
            matches!(p.place, citygen::Place::Floor { .. }),
            "{:?}",
            p.place
        );
        assert!(
            crate::render::map::MapRef::for_place(
                b.city.as_ref().unwrap(),
                &b.interiors,
                &b.constructs,
                p.place,
                b.minute_of_day(),
            )
            .is_some(),
            "the flat's floor is there to draw"
        );
        // Both opened on the map the same way, as a load opens it.
        let mut drawn = Vec::new();
        for st in [&mut a, &mut b] {
            st.show_fps = false;
            let mut map = MapScreen::walking(st);
            map.settle(st, size);
            drawn.push(buffer_text(&crate::screens::shots::draw_stack(
                st,
                &[&map],
                size,
            )));
        }
        assert_eq!(
            drawn[1], drawn[0],
            "the restored map draws as the saved one"
        );
    }

    #[test]
    fn mail_and_feeds_come_back_as_they_were() {
        use crate::mail_mark::testutil::land;
        let dir = Dir::new("mail");
        let mut a = AppState::new();
        a.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed: 8,
                size: citygen::SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        let _ = MapScreen::walking(&mut a);
        crate::missions::begin(&mut a);
        assert_eq!(a.missions.feeds.len(), 1, "the welcome feed is queued");
        for subject in ["one", "two", "three"] {
            land(&mut a, "Mezz", subject);
        }
        crate::mail_mark::notice(&mut a);
        assert_eq!(a.mail_seen, 3);
        let a = running(a);
        let path = write(&dir.0, &a).unwrap();
        let mut b = reopened(&path);
        assert_eq!(b.mail_seen, 3, "the mails were announced");
        let announced = |st: &AppState| {
            st.log
                .iter()
                .filter(|e| e.text.contains(" · mail · "))
                .count()
        };
        assert_eq!(announced(&b), 3);
        let mut map = MapScreen::walking(&mut b);
        let mut feeds = 0;
        for _ in 0..4 {
            if matches!(map.update(&mut b, &[]), ScreenCmd::Push { .. }) {
                feeds += 1;
            }
            b.tick += 1;
        }
        assert_eq!(feeds, 1, "the feed opens once");
        assert!(b.missions.feeds.is_empty());
        assert_eq!(announced(&b), 3, "no mail is announced again");
    }

    #[test]
    fn a_broken_file_lists_as_unreadable() {
        let dir = Dir::new("broken");
        let st = small_run();
        let good = write(&dir.0, &st).unwrap();
        let bytes = std::fs::read(&good).unwrap();
        std::fs::write(dir.0.join("half.run"), &bytes[..bytes.len() / 2]).unwrap();
        let header_end = 10 + u32::from_le_bytes(bytes[6..10].try_into().unwrap()) as usize;
        std::fs::write(dir.0.join("head.run"), &bytes[..header_end / 2]).unwrap();
        let noise: Vec<u8> = (0..4096u64)
            .map(|i| citygen::rng::hash64(i, 0xBAD) as u8)
            .collect();
        std::fs::write(dir.0.join("noise.run"), &noise).unwrap();
        std::fs::write(dir.0.join("empty.run"), []).unwrap();
        let mut newer = bytes.clone();
        newer[4..6].copy_from_slice(&2u16.to_le_bytes());
        std::fs::write(dir.0.join("newer.run"), &newer).unwrap();
        // A header that says it is four gigabytes long, in a file of a few bytes.
        let mut huge = bytes[..PREAMBLE].to_vec();
        huge[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
        huge.extend_from_slice(b"short");
        std::fs::write(dir.0.join("huge.run"), &huge).unwrap();
        // A header whose body would run past the end of anything.
        let lying = Head {
            run: header(&good).unwrap(),
            body: u64::MAX,
        };
        let lying = rmp_serde::to_vec_named(&lying).unwrap();
        let mut liar = bytes[..PREAMBLE].to_vec();
        liar[6..10].copy_from_slice(&(lying.len() as u32).to_le_bytes());
        liar.extend_from_slice(&lying);
        std::fs::write(dir.0.join("liar.run"), &liar).unwrap();
        std::fs::write(dir.0.join("notes.txt"), b"not a run").unwrap();

        let listed = crate::runs::list(&dir.0);
        let reason = |name: &str| {
            listed
                .iter()
                .find(|r| r.path.file_name().unwrap() == name)
                .unwrap_or_else(|| panic!("{name} is listed"))
                .header
                .clone()
        };
        assert_eq!(listed.len(), 8, "every .run, and only those");
        assert!(reason(good.file_name().unwrap().to_str().unwrap()).is_ok());
        for name in [
            "half.run",
            "head.run",
            "noise.run",
            "empty.run",
            "huge.run",
            "liar.run",
        ] {
            assert!(reason(name).is_err(), "{name} lists as unreadable");
            assert!(
                read(&dir.0.join(name), book()).is_err(),
                "{name} doesn't open"
            );
        }
        assert_eq!(reason("newer.run").unwrap_err(), "this build can't open it");
        assert!(read(&dir.0.join("newer.run"), book()).is_err());
        // The living list keeps the good one first, the unreadable ones after it.
        let living = crate::runs::living(&dir.0);
        assert_eq!(living.len(), 8);
        assert!(living[0].header.is_ok());
        assert!(living[1..].iter().all(|r| r.header.is_err()));
        assert!(crate::runs::dead(&dir.0).is_empty());
    }

    #[test]
    fn the_header_reads_without_the_body() {
        let dir = Dir::new("header");
        let st = small_run();
        let path = write(&dir.0, &st).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let header_end = 10 + u32::from_le_bytes(bytes[6..10].try_into().unwrap()) as usize;
        for (i, b) in bytes[header_end..].iter_mut().enumerate() {
            *b = citygen::rng::hash64(i as u64, 7) as u8;
        }
        std::fs::write(&path, &bytes).unwrap();
        let h = header(&path).expect("the header reads");
        assert_eq!(h.city, st.city.as_ref().unwrap().name);
        assert!(read(&path, book()).is_err(), "the body is garbage");
    }

    #[test]
    fn a_new_header_keeps_the_body() {
        let dir = Dir::new("rewrite");
        let st = small_run();
        let path = write(&dir.0, &st).unwrap();
        let mut h = header(&path).unwrap();
        h.death = Some(crate::runs::Death {
            cause: "black ICE at the tower".to_string(),
            day: 3,
            minute: 7 * 60 + 40,
            at: 1_793_789_040,
            lasted: 3 * 1440,
        });
        rewrite_header(&path, &h).unwrap();
        assert_eq!(header(&path).unwrap(), h);
        let mut b = AppState::new();
        let (_, s, lost) = read(&path, &b.book).expect("the body still reads");
        restore(&mut b, s);
        after_restore(&mut b, lost);
        assert!(same(&snapshot(&st), &snapshot(&b)));
        assert!(crate::runs::living(&dir.0).is_empty());
        assert_eq!(crate::runs::dead(&dir.0).len(), 1);
    }

    #[test]
    fn the_lists_put_the_newest_first() {
        let dir = Dir::new("order");
        let st = small_run();
        let mut paths = Vec::new();
        for (n, saved) in [(1u64, 100u64), (2, 300), (3, 200)] {
            let mut s = small_run_like(&st, n);
            s.character.handle = format!("r{n}");
            let path = write(&dir.0, &s).unwrap();
            let mut h = header(&path).unwrap();
            h.saved = saved;
            if n == 3 {
                h.death = Some(crate::runs::Death {
                    cause: "shot".to_string(),
                    day: 1,
                    minute: 0,
                    at: saved,
                    lasted: 40,
                });
            }
            rewrite_header(&path, &h).unwrap();
            paths.push(path);
        }
        let handles = |runs: Vec<crate::runs::RunInfo>| -> Vec<String> {
            runs.into_iter().map(|r| r.header.unwrap().handle).collect()
        };
        assert_eq!(handles(crate::runs::living(&dir.0)), ["r2", "r1"]);
        assert_eq!(handles(crate::runs::dead(&dir.0)), ["r3"]);
    }

    /// `st`'s run under another id, so it writes a file of its own.
    fn small_run_like(st: &AppState, n: u64) -> AppState {
        let mut s = AppState::new();
        s.enter_city(st.city.clone().unwrap());
        s.run = Some(RunMeta {
            id: RunId(n),
            started: 0,
            born: 0,
        });
        s
    }

    #[test]
    fn a_new_state_saves_nowhere() {
        let st = AppState::new();
        assert_eq!(st.saves, None);
        assert_eq!(st.run, None);
        let dir = Dir::new("nowhere");
        assert!(write(&dir.0, &st).is_err(), "no run, no file");
        assert!(crate::runs::list(&dir.0).is_empty());
        assert!(crate::runs::list(&dir.0.join("missing")).is_empty());
    }

    #[test]
    fn a_run_id_is_sixteen_hex_digits() {
        assert_eq!(RunId(0xbeef).to_string(), "000000000000beef");
        assert_eq!(RunId::fresh(9).to_string().len(), 16);
        assert_eq!(crate::runs::default_dir(), PathBuf::from("runs"));
    }

    #[test]
    fn a_restored_run_starts_the_deck_over() {
        let dir = Dir::new("screens");
        let st = small_run();
        let path = write(&dir.0, &st).unwrap();
        let mut b = AppState::new();
        b.deck_part = 4;
        b.deck_jobs_sel = Some(crate::screens::jobs_tab::Entry::Mission(1));
        b.rain = false;
        b.look_at = Some(citygen::Pos::new(3, 4));
        b.jump_to = Some(citygen::Pos::new(5, 6));
        b.save_due = true;
        let (_, s, _) = read(&path, book()).unwrap();
        restore(&mut b, s);
        assert_eq!(b.deck_part, 0, "the deck starts over, as in a new city");
        assert_eq!(b.deck_jobs_sel, None);
        assert_eq!(
            (b.look_at, b.jump_to),
            (None, None),
            "no map request carries over"
        );
        assert!(!b.save_due, "nor a save the last run asked for");
        assert!(!b.rain, "a setting stays as it was");
    }
}
