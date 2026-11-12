//! The run file: the magic `SZRN`, a little-endian `u16` format version, a
//! little-endian `u32` header length, the header (MessagePack with named fields), then
//! the body: the snapshot, the same, gzipped. Written beside itself and renamed over the
//! old file, so a crash mid-write keeps the last good save.

use super::snapshot::{Snapshot, snapshot};
use super::{RunHeader, RunId, describe, ids};
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

/// The file of the run `id` in `dir`: `<dir>/<id>.run`.
pub fn path(dir: &Path, id: RunId) -> PathBuf {
    dir.join(format!("{id}.run"))
}

/// Writes the run in `st` to its file in `dir`, the folder made if need be; the path.
/// Changes nothing in `st`. A state with no run has nothing to write.
pub fn write(dir: &Path, st: &AppState) -> Result<PathBuf> {
    write_as(dir, st, &describe::header(st, None))
}

/// `write`, with `header` for the file's header: the flatline's, marked with the death.
pub fn write_as(dir: &Path, st: &AppState, header: &RunHeader) -> Result<PathBuf> {
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
        run: header.clone(),
        body: body.len() as u64,
    };
    std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    let path = path(dir, run.id);
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
    patiently(|| std::fs::rename(&tmp, path)).with_context(|| format!("writing {}", path.display()))
}

/// How often a refused rename is tried again, and how long apart: on Windows a virus
/// scanner can hold the file just closed for a moment, and the rename is refused
/// (access denied, a sharing violation) until it lets go.
const RENAME_RETRIES: u32 = 2;
const RENAME_WAIT: std::time::Duration = std::time::Duration::from_millis(50);

/// `rename` run, and while it fails run again, `RENAME_RETRIES` more times at most,
/// `RENAME_WAIT` apart: its last result.
fn patiently(mut rename: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
    let mut retries = 0;
    loop {
        match rename() {
            Err(_) if retries < RENAME_RETRIES => {
                retries += 1;
                std::thread::sleep(RENAME_WAIT);
            }
            done => return done,
        }
    }
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

/// Puts `header` in place of the file's own, its body kept byte for byte: how the
/// tests stamp a run as saved or flatlined when they like.
#[cfg(test)]
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

    /// You on foot on the street at `at`.
    fn on_foot(st: &mut AppState, at: citygen::Pos) {
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Stratum(citygen::StratumKind::Street);
        p.pos = at;
        p.body = None;
    }

    /// A camera looped from the Net is as blind once the run is reopened, and stays so as
    /// the game goes on: it sees you standing in front of it no more than before the
    /// save, and its minute warms nothing.
    #[test]
    fn a_looped_camera_stays_blind_after_a_reopen() {
        use crate::net::tests::{crack_to_landing, under_kiroflux};
        use citygen::{Place, Pos, StratumKind};
        let (mut a, b) = under_kiroflux();
        let cam = crate::cameras::street_cameras(a.city.as_ref().unwrap())
            .find(|c| c.building == b)
            .expect("Kiroflux Mart's street camera");
        let ahead = cam.at.step(cam.facing).step(cam.facing);
        on_foot(&mut a, ahead);
        assert_eq!(crate::cameras::seeing(&a), vec![cam], "it sees you there");
        // Held from the lane under its wall and looped.
        let at = a
            .city
            .as_ref()
            .unwrap()
            .net
            .node(cam.node.city().unwrap())
            .cyber;
        {
            let p = a.player.as_mut().unwrap();
            p.place = Place::Stratum(StratumKind::Net);
            p.pos = Pos::new(at.x, at.y + 1);
            p.body = Some((Place::Stratum(StratumKind::Street), ahead));
        }
        crack_to_landing(&mut a, game::HackTarget::Node(cam.node.city().unwrap()));
        crate::holds::use_verb(&mut a, 'o').unwrap();
        on_foot(&mut a, ahead);
        assert!(crate::cameras::seeing(&a).is_empty(), "looped");
        let heat = |st: &AppState| st.net.traces.get(&b).map_or(0, |t| t.heat);
        let warm = heat(&a);

        let dir = Dir::new("looped");
        let a = running(a);
        let path = write(&dir.0, &a).unwrap();
        let mut st = reopened(&path);
        let now = st.net.now(&st);
        assert!(crate::holds::looped(&st, cam.node, now), "the loop is kept");
        assert_eq!(st.net.holds, a.net.holds, "until the same minute");
        assert!(crate::cameras::seeing(&st).is_empty(), "still blind");
        // The game goes on in front of it: two game minutes of the map, the cameras'
        // minute run in each.
        st.show_fps = false;
        let mut map = MapScreen::resumed(&mut st);
        let opened = st.tick;
        while st.net.now(&st) < now + 2 {
            st.tick += 1;
            map.update(&mut st, &[]);
            assert!(st.tick < opened + 3_000, "the clock runs");
        }
        assert_eq!(st.player.as_ref().unwrap().pos, ahead);
        assert!(heat(&st) <= warm, "the crack's heat cools; no eye warms it");
        let cooled = heat(&st);
        st.net.eyes_minute = None;
        crate::cameras::tick(&mut st);
        assert!(crate::cameras::seeing(&st).is_empty(), "still blind");
        assert_eq!(heat(&st), cooled, "nothing seen, nothing warmed");
        // It is the loop: the hold gone, the camera sees you where you stand.
        st.net.holds.clear();
        assert_eq!(crate::cameras::seeing(&st), vec![cam]);
    }

    /// A site's construct comes back whole with the run, you on its Net where you stood:
    /// its levels and devices, its ICE (where each piece stands, what's lifted, each
    /// piece's state), the firewall's broken locks, and the hacks, holds and feed that name
    /// its nodes. Its Net isn't saved, and is worked out again the same from the saved plan
    /// and the copy's layer.
    #[test]
    fn a_site_construct_and_its_firewall_come_back() {
        use crate::sitenet::testutil::{device, hack, office_copy, on_hub};
        use citygen::{NetNode, NodeKind, Place};
        use game::{HackTarget, Op};
        let (mut a, b) = office_copy(210);
        on_hub(&mut a, b, 0);
        let now = a.net.now(&a);
        a.net.break_lock(b, 1, now);
        // A patrol a step along its beat, the white at G's hub lifted.
        let walker = {
            let k = a.constructs.get_mut(b).unwrap();
            let i = (k.guards.iter())
                .position(|g| g.beat.len() > 1)
                .expect("a patrol at 210");
            let to = k.guards[i].beat[1];
            k.move_guard(i, to);
            k.set_guard(0, false);
            i
        };
        let k = a.constructs.site(b).unwrap().clone();
        assert!(k.guards[walker].pos != k.guards[walker].post && !k.guards[0].standing);
        a.net.ice_of(&k)[walker].step = 1;
        // A door's lock cracked, a camera held and its feed open.
        let door = device(&a, b, NodeKind::DoorLock).node;
        hack(&mut a, door, NodeKind::DoorLock, Op::Crack);
        let cam = NetNode::Site(device(&a, b, NodeKind::Camera).node);
        crate::holds::take(&mut a, cam, crate::owners::Owner::Building(b), now);
        a.net.feed = Some(cam);
        let net = crate::sitenet::net_of(&a, b).expect("a site's Net");

        let dir = Dir::new("site-net");
        let a = running(a);
        let st = reopened(&write(&dir.0, &a).unwrap());
        let back = st
            .constructs
            .site(b)
            .expect("the site's construct comes back");
        assert!(*back == k, "the construct comes back whole");
        assert!(!back.levels.is_empty() && !back.devices.is_empty());
        assert_eq!(back.guards[walker].pos, k.guards[walker].pos);
        assert!(!back.guards[0].standing, "still lifted");
        assert_eq!(st.net.firewalls, a.net.firewalls);
        assert_eq!(
            crate::sitenet::broken(&st, b),
            crate::sitenet::broken(&a, b)
        );
        assert!(st.net.ice.contains_key(&b), "its ICE states");
        assert_eq!(st.net.ice.get(&b), a.net.ice.get(&b));
        assert!(
            st.net
                .hacks
                .iter()
                .any(|h| h.target == HackTarget::Site(door))
        );
        assert_eq!(st.net.hacks, a.net.hacks);
        assert!(
            crate::holds::held(&st, cam, now).is_some(),
            "the camera still held"
        );
        assert_eq!(st.net.holds, a.net.holds);
        assert_eq!(st.net.feed, Some(cam), "its feed open");
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        assert_eq!(p.body, a.player.as_ref().unwrap().body, "the body outside");
        assert_eq!(
            crate::sitenet::net_of(&st, b).as_deref(),
            Some(&*net),
            "the Net worked out again the same"
        );
    }

    /// A save standing in a site's old construct (one made before the site had a Net of
    /// its own) opens and plays there, the old construct kept while you stand in it; out
    /// through its gate, the gate in is the firewall's (Ruling 13 of the plan
    /// `docs/superpowers/plans/2026-11-11-site-net.md`).
    #[test]
    fn a_save_inside_a_sites_old_construct_carries_on_until_you_step_out() {
        use citygen::{Place, StratumKind};
        let (mut a, b) = crate::sitenet::testutil::office_plain();
        assert!(crate::sitenet::is_site(&a, b));
        let city = a.city.clone().unwrap();
        // Today's construct, as an old save holds it.
        let entry = a.constructs.ensure(&city, b).unwrap().entry;
        let mut p = crate::walker::Player::at(Place::Construct(b), entry);
        p.body = Some((
            Place::Stratum(StratumKind::Street),
            city.building(b).footprint.center(),
        ));
        a.player = Some(p);
        let old = a.constructs.get(b).unwrap().clone();
        assert!(!old.is_site());

        let dir = Dir::new("old-construct");
        let a = running(a);
        let st = reopened(&write(&dir.0, &a).unwrap());
        assert_eq!(st.player.as_ref().unwrap().place, Place::Construct(b));
        assert!(
            st.constructs.get(b) == Some(&old),
            "the old construct is kept"
        );
        let mut st = played(st, 12);
        assert_eq!(
            st.player.as_ref().unwrap().place,
            Place::Construct(b),
            "it carries on"
        );
        assert!(
            st.constructs.get(b).is_some_and(|k| !k.is_site()),
            "still the old construct"
        );
        crate::constructs::leave_construct(&mut st).unwrap();
        assert!(
            crate::constructs::enter_construct(&mut st, b)
                .unwrap_err()
                .starts_with("FIREWALL")
        );
    }

    /// `st` back on the map as a load puts it (`MapScreen::resumed`) and played `frames`
    /// frames of the game's own loop, each drawn.
    fn played(mut st: AppState, frames: usize) -> AppState {
        use crate::app::Runner;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        st.show_fps = false;
        let map = MapScreen::resumed(&mut st);
        let mut runner = Runner::new(Box::new(map), st);
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for _ in 0..frames {
            assert!(runner.frame(&mut term, &[]).unwrap(), "it plays");
        }
        runner.st
    }

    /// A copy lost while you stood on its Net leaves its construct as it stood (nothing is
    /// dropped under you), so a save then holds a site's construct its plan and its copy
    /// no longer make. It opens and plays there, jacking out works, and the construct
    /// stays the building's, through the firewall too, until a copy on it next goes live.
    #[test]
    fn a_save_on_a_lost_copys_net_opens_and_plays() {
        use crate::sitenet::testutil::{at_the_gate, office_copy, on_hub};
        use citygen::Place;
        let (mut a, b) = office_copy(190);
        on_hub(&mut a, b, 0);
        let made = a.constructs.site(b).unwrap().clone();
        // As a save from before copies kept their layer reads: lost at once.
        a.interiors.copy_mut(b).unwrap().layer = None;
        crate::venues::tick(&mut a);
        assert!(a.interiors.copy(b).is_none(), "lost");
        assert!(a.constructs.site(b) == Some(&made), "left as it stood");

        let dir = Dir::new("lost-copy");
        let a = running(a);
        let st = reopened(&write(&dir.0, &a).unwrap());
        assert!(
            st.constructs.site(b) == Some(&made),
            "it comes back as saved"
        );
        assert_eq!(
            st.player.as_ref().unwrap().place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        let mut st = played(st, 12);
        assert_eq!(
            st.player.as_ref().unwrap().place,
            Place::SiteNet {
                building: b,
                level: 0
            },
            "it plays on there"
        );
        crate::net::jack_out(&mut st).unwrap();
        assert_eq!(
            st.player.as_ref().unwrap().place,
            a.player.as_ref().unwrap().body.unwrap().0
        );
        assert!(
            st.constructs.site(b) == Some(&made),
            "kept till the next copy"
        );
        // Back in through the firewall, every lock broken: the same construct.
        at_the_gate(&mut st, b);
        let now = st.net.now(&st);
        for lock in 0..crate::sitenet::broken(&st, b).len() {
            st.net.break_lock(b, lock as u8, now);
        }
        crate::constructs::enter_construct(&mut st, b).unwrap();
        assert!(matches!(
            st.player.as_ref().unwrap().place,
            Place::SiteNet { building, .. } if building == b
        ));
        assert!(st.constructs.site(b) == Some(&made));
        let st = played(st, 6);
        assert!(st.player.as_ref().unwrap().jacked_in());
    }

    /// A district's lamps cut from its lamp grid are still out once the run is reopened,
    /// until the minute they were cut till: the street's dark isn't saved, but is made
    /// again from what is.
    #[test]
    fn the_lights_stay_out_after_a_reopen() {
        use crate::net::tests::{crack_to_landing, under_kiroflux};
        use citygen::{NodeKind, Place, Pos, StratumKind};
        let (mut a, b) = under_kiroflux();
        // Ten at night: the lamps burn.
        {
            let city = a.city.as_ref().unwrap();
            let w = a.world.as_mut().unwrap();
            while w.minute_of_day() != 22 * 60 {
                let t = w.tick() + sim::TICKS_PER_MINUTE;
                w.advance(city, t);
            }
        }
        let city = a.city.as_ref().unwrap();
        let d = city.buildings[b.index()].district;
        let grid = city
            .net
            .nodes
            .iter()
            .find(|n| {
                n.kind == NodeKind::LampGrid
                    && crate::owners::owner_of(city, n.id)
                        == Some(crate::owners::Owner::District(d))
            })
            .expect("the district's lamp grid");
        let (node, at) = (grid.id, grid.cyber);
        let street = city.stratum(StratumKind::Street).unwrap();
        let lamp = street
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| street.lamps[*p] > 0 && street.tiles[*p].district == Some(d))
            .expect("a lamp-lit tile in the district");
        let lit = street.emitted(lamp, 22 * 60);
        {
            let p = a.player.as_mut().unwrap();
            p.place = Place::Stratum(StratumKind::Net);
            p.pos = Pos::new(at.x + 1, at.y);
        }
        crack_to_landing(&mut a, game::HackTarget::Node(node));
        crate::holds::use_verb(&mut a, 'o').unwrap();
        let street_of = |st: &AppState| {
            st.city
                .as_ref()
                .unwrap()
                .stratum(StratumKind::Street)
                .unwrap()
                .clone()
        };
        assert!(!street_of(&a).lamps_lit(lamp, 22 * 60), "cut");

        let dir = Dir::new("lights-out");
        let a = running(a);
        let path = write(&dir.0, &a).unwrap();
        let mut st = reopened(&path);
        assert_eq!(st.net.lights_out, a.net.lights_out, "till the same minute");
        let minute = st.minute_of_day();
        assert_eq!(minute, 22 * 60);
        let street = street_of(&st);
        assert!(street.dark.contains(&d), "the dark made again");
        assert!(!street.lamps_lit(lamp, minute), "the lamps still out");
        assert!(street.emitted(lamp, minute) < lit);
        // A frame's clock keeps them out while the minute lasts.
        crate::holds::tick(&mut st);
        assert!(!street_of(&st).lamps_lit(lamp, minute));
        // And the minute come, they burn again.
        st.net.lights_out.values_mut().for_each(|u| *u = 0);
        crate::holds::tick(&mut st);
        assert_eq!(street_of(&st).emitted(lamp, minute), lit);
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

    /// How long a save holds the frame, on the medium seed-8 morning after: `write`
    /// whole, and its parts one by one (the snapshot to MessagePack, the gzip, the
    /// file written, synced and renamed). Run in release:
    /// `cargo test -p strata-zero --release --target-dir target/smoke -- --ignored
    /// a_save_is_timed --nocapture`.
    #[test]
    #[ignore = "a measurement, in release"]
    fn a_save_is_timed() {
        use std::time::Instant;
        const RUNS: usize = 7;
        let dir = Dir::new("timed");
        let st = running(crate::screens::shots::seren_has_written());
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        let median = |mut v: Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        let (mut whole, mut encode, mut gzip, mut put_ms) = (vec![], vec![], vec![], vec![]);
        let (mut raw_len, mut body_len) = (0, 0);
        for _ in 0..RUNS {
            let t = Instant::now();
            write(&dir.0, &st).unwrap();
            whole.push(ms(t));

            let t = Instant::now();
            let raw = ids::with_book(&st.book, || rmp_serde::to_vec_named(&snapshot(&st))).unwrap();
            encode.push(ms(t));
            let t = Instant::now();
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            gz.write_all(&raw).unwrap();
            let body = gz.finish().unwrap();
            gzip.push(ms(t));
            let head = Head {
                run: describe::header(&st, None),
                body: body.len() as u64,
            };
            let t = Instant::now();
            put(&path(&dir.0, st.run.unwrap().id), &head, &body).unwrap();
            put_ms.push(ms(t));
            (raw_len, body_len) = (raw.len(), body.len());
        }
        eprintln!(
            "a save of the medium seed-8 morning after, median of {RUNS}: write {:.1} ms \
             · encode {:.1} ms ({raw_len} bytes) · gzip {:.1} ms ({body_len} bytes) \
             · write+fsync+rename {:.1} ms",
            median(whole),
            median(encode),
            median(gzip),
            median(put_ms),
        );
    }

    #[test]
    fn a_refused_rename_is_tried_again() {
        let refused = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        // Held for two tries, then let go: it goes.
        let mut tries = 0;
        let went = patiently(|| {
            tries += 1;
            if tries < 3 { Err(refused()) } else { Ok(()) }
        });
        assert!(went.is_ok());
        assert_eq!(tries, 3);
        // Never let go: three tries, then the refusal.
        let mut tries = 0;
        let went = patiently(|| {
            tries += 1;
            Err(refused())
        });
        assert_eq!(
            went.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(tries, 1 + RENAME_RETRIES);
        // One that goes at once is tried once.
        let mut tries = 0;
        assert!(
            patiently(|| {
                tries += 1;
                Ok(())
            })
            .is_ok()
        );
        assert_eq!(tries, 1);
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
        b.levelled = Some(2);
        b.save_due = true;
        let (_, s, _) = read(&path, book()).unwrap();
        restore(&mut b, s);
        assert_eq!(b.deck_part, 0, "the deck starts over, as in a new city");
        assert_eq!(b.deck_jobs_sel, None);
        assert_eq!(
            (b.look_at, b.jump_to, b.levelled),
            (None, None, None),
            "no map request carries over"
        );
        assert!(!b.save_due, "nor a save the last run asked for");
        assert!(!b.rain, "a setting stays as it was");
    }
}
