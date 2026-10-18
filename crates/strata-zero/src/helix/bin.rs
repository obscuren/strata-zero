//! The world behind HELIX/OS: the building as `/dev`, the district as `/net`, the
//! old terminal verbs as programs. `AppIo` is the `Io` the game gives the shell.

use super::hosts::{self, ensure_host, hostname, user_for};
use super::shell::{HostId, Io, Output, Shell};
use super::vfs::{Dev, Vfs};
use crate::access::{self, Tier, building_label, tier_on};
use crate::app::AppState;
use crate::ui::device::Look;
use citygen::BuildingId;

#[cfg(test)]
use super::vfs::Kind;

/// The shell's window on the game: every device and program reads and writes `st`.
pub struct AppIo<'a> {
    pub st: &'a mut AppState,
}

/// The programs the world adds to the builtins.
const PROGRAMS: &[&str] = &[
    "scan", "ssh", "connect", "mail", "journal", "door", "lift", "cam", "alarm", "plan",
];

/// A shell logged in on `host`: the user the host's tier gives you, in their home.
pub fn new_shell(st: &mut AppState, host: HostId) -> Shell {
    let mut shell = Shell::new(host, "", "");
    login(st, &mut shell, host);
    shell
}

/// Moves `shell` onto `host`, logged in as the user your standing there allows: in
/// their home, or at `/` when the home is the shared `/tmp` (guest, `user`).
fn login(st: &mut AppState, shell: &mut Shell, host: HostId) {
    let user = user_for(st, host);
    let home = ensure_host(st, host)
        .users
        .iter()
        .find(|u| u.name == user)
        .map_or_else(|| "/tmp".to_string(), |u| u.home.clone());
    shell.host = host;
    shell.user = user;
    shell.cwd = if home == "/tmp" {
        "/".to_string()
    } else {
        home.clone()
    };
    shell.home = home;
}

fn lines(lines: Vec<String>) -> Output {
    Output {
        lines,
        ..Output::default()
    }
}

fn one(line: impl Into<String>) -> Output {
    lines(vec![line.into()])
}

impl AppIo<'_> {
    /// The building a host stands for: itself, or the rig's own block.
    fn block(&self, host: HostId) -> Option<BuildingId> {
        match host {
            HostId::Building(b) => Some(b),
            HostId::Rig => self.st.home.as_ref().map(|h| h.home.building),
            HostId::District(_) => None,
        }
    }

    /// The hosts a scan from `host` lists: a district's constructs, the rig's block.
    fn net_of(&self, host: HostId) -> Vec<BuildingId> {
        match host {
            HostId::District(d) => access::scannable(self.st, d),
            HostId::Rig => self.block(host).into_iter().collect(),
            HostId::Building(_) => Vec::new(),
        }
    }

    /// The building a program acts on, if the shell's standing there allows it:
    /// `dev` is the device it would write, root's alone when `admin`.
    fn allow(
        &mut self,
        shell: &Shell,
        cmd: &str,
        dev: &str,
        admin: bool,
    ) -> Result<BuildingId, String> {
        let Some(b) = self.block(shell.host) else {
            return Err(format!("{cmd}: NOT CONNECTED · SSH A BUILDING FIRST"));
        };
        let denied = || format!("{cmd}: {dev}: Permission denied");
        if shell.host == HostId::Rig {
            // the rig is at least a resident on its own block
            let have = tier_on(self.st, b).max(Tier::User);
            return if have >= if admin { Tier::Admin } else { Tier::User } {
                Ok(b)
            } else {
                Err(denied())
            };
        }
        let fs = &self.fs(shell.host);
        let node = fs
            .resolve("/", "/", dev)
            .or_else(|_| fs.resolve("/", "/", "/dev/door"));
        match node {
            Ok(id) if fs.can_write(id, &shell.user) => Ok(b),
            _ => Err(denied()),
        }
    }

    fn scan(&mut self, shell: &Shell) -> Result<Output, String> {
        let list = self.net_of(shell.origin);
        if list.is_empty() {
            return Ok(one("NO CONSTRUCTS ON THIS SUBNET"));
        }
        Ok(lines(
            list.iter()
                .enumerate()
                .map(|(i, b)| access::scan_row(self.st, i, *b))
                .collect(),
        ))
    }

    fn ssh(&mut self, shell: &mut Shell, args: &[String]) -> Result<Output, String> {
        let [what] = args else {
            return Err("usage: ssh <host>".to_string());
        };
        let what = what.to_lowercase();
        let list = self.net_of(shell.origin);
        // the names /net lists, twins told apart
        let names: Vec<String> = match shell.origin {
            HostId::District(d) => hosts::net_names(self.st, d)
                .into_iter()
                .map(|(_, n)| n)
                .collect(),
            _ => list
                .iter()
                .map(|b| hostname(self.st, HostId::Building(*b)))
                .collect(),
        };
        let pick: Vec<usize> = if let Ok(n) = what.parse::<usize>() {
            n.checked_sub(1)
                .filter(|i| *i < list.len())
                .into_iter()
                .collect()
        } else if let Some(i) = names.iter().position(|n| *n == what) {
            vec![i]
        } else {
            (0..names.len())
                .filter(|i| names[*i].starts_with(&what))
                .collect()
        };
        match pick.as_slice() {
            [] => Err(format!("ssh: {what}: no such host")),
            [i] => {
                let b = list[*i];
                login(self.st, shell, HostId::Building(b));
                Ok(one(format!(
                    "CONNECTED TO {} · CLEARANCE {}",
                    building_label(self.st, b),
                    tier_on(self.st, b).label()
                )))
            }
            many => Err(format!(
                "ssh: {what}: ambiguous: {}",
                many.iter()
                    .map(|i| names[*i].as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn exit(&mut self, shell: &mut Shell) -> Option<Result<Output, String>> {
        if shell.host == shell.origin {
            return None;
        }
        login(self.st, shell, shell.origin);
        Some(Ok(one("CONNECTION CLOSED")))
    }

    fn mail(&mut self, shell: &Shell, args: &[String]) -> Result<Output, String> {
        if shell.host != HostId::Rig {
            return Err("mail: no mailbox on this host".to_string());
        }
        match args {
            [] if self.st.journal.mails.is_empty() => Ok(one("NO MAIL")),
            [] => Ok(lines(access::mail_inbox(self.st))),
            [n] => {
                let i = n
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .ok_or_else(|| "usage: mail [n]".to_string())?;
                let mut out = access::mail_open(self.st, i).map_err(|e| format!("mail: {e}"))?;
                out[0] = out[0].to_uppercase();
                out[1] = out[1].to_uppercase();
                Ok(lines(out))
            }
            _ => Err("usage: mail [n]".to_string()),
        }
    }

    /// `<cmd> <word>` on the building: the permission, then the action.
    fn act(
        &mut self,
        shell: &Shell,
        cmd: &str,
        word: &str,
        args: &[String],
        dev: &str,
        admin: bool,
    ) -> Result<Output, String> {
        if args.len() != 1 || args[0] != word {
            return Err(format!("usage: {cmd} {word}"));
        }
        let b = self.allow(shell, cmd, dev, admin)?;
        let done = match cmd {
            "door" => access::release_doors(self.st, b),
            "lift" => access::answer_lifts(self.st, b),
            "cam" => access::loop_cams(self.st, b),
            _ => access::alarm_off(self.st, b),
        };
        done.map(one).map_err(|e| format!("{cmd}: {e}"))
    }

    fn render_passwd(&mut self, host: HostId) -> Vec<String> {
        ensure_host(self.st, host)
            .users
            .iter()
            .enumerate()
            .map(|(i, u)| {
                format!(
                    "{}:x:{}:{}:{}:/bin/helix",
                    u.name,
                    if u.name == "root" { 0 } else { 1000 + i },
                    u.home,
                    u.display
                )
            })
            .collect()
    }
}

impl Io for AppIo<'_> {
    fn fs(&mut self, host: HostId) -> &mut Vfs {
        &mut ensure_host(self.st, host).fs
    }

    fn hostname(&self, host: HostId) -> String {
        hostname(self.st, host)
    }

    fn os_version(&self, host: HostId) -> String {
        Look::pick(self.st, hosts::device_of(host)).style.os_version
    }

    fn read_dev(&mut self, host: HostId, dev: &Dev, _user: &str) -> Result<Vec<String>, String> {
        match dev {
            Dev::Journal => return Ok(access::journal_lines(self.st)),
            Dev::Mail(i) => return access::mail_open(self.st, *i),
            Dev::Passwd => return Ok(self.render_passwd(host)),
            Dev::NetHost(b) => {
                let i = self.net_of(host).iter().position(|x| x == b).unwrap_or(0);
                return Ok(vec![access::scan_row(self.st, i, *b)]);
            }
            _ => {}
        }
        let Some(b) = self.block(host) else {
            return Err("helix: no such device".to_string());
        };
        match dev {
            Dev::Door => Ok(vec![access::door_state(self.st, b)]),
            Dev::Lift => Ok(vec![access::lift_state(self.st, b)]),
            Dev::Cam(i) => Ok(vec![access::cam_state(self.st, b, *i)]),
            Dev::Alarm => Ok(vec![access::alarm_state(self.st, b)]),
            Dev::Plan => access::load_plan(self.st, b).map(|l| vec![l]),
            Dev::AuthLog => Ok(access::log_lines(self.st, b)),
            Dev::NetLog => Ok(access::trace_lines(self.st, b)),
            _ => Err("helix: no such device".to_string()),
        }
    }

    fn write_dev(
        &mut self,
        host: HostId,
        dev: &Dev,
        _user: &str,
        text: &str,
    ) -> Result<Vec<String>, String> {
        let Some(b) = self.block(host) else {
            return Err("helix: no such device".to_string());
        };
        let word = text.trim();
        let (name, done) = match (dev, word) {
            (Dev::Door, "release") => ("door", access::release_doors(self.st, b)),
            (Dev::Lift, "answer") => ("lift", access::answer_lifts(self.st, b)),
            (Dev::Cam(i), "loop") => ("cam", access::loop_cam(self.st, b, *i)),
            (Dev::Alarm, "off") => ("alarm", access::alarm_off(self.st, b)),
            (Dev::Door, _) => return Err("door: unknown request".to_string()),
            (Dev::Lift, _) => return Err("lift: unknown request".to_string()),
            (Dev::Cam(_), _) => return Err("cam: unknown request".to_string()),
            (Dev::Alarm, _) => return Err("alarm: unknown request".to_string()),
            _ => return Err("helix: device is read-only".to_string()),
        };
        done.map(|l| vec![l]).map_err(|e| format!("{name}: {e}"))
    }

    fn program(
        &mut self,
        shell: &mut Shell,
        name: &str,
        args: &[String],
        _stdin: &[String],
    ) -> Option<Result<Output, String>> {
        Some(match name {
            "scan" => self.scan(shell),
            "ssh" | "connect" => self.ssh(shell, args),
            "exit" => return self.exit(shell),
            "mail" => self.mail(shell, args),
            "journal" if shell.host == HostId::Rig => Ok(lines(access::journal_lines(self.st))),
            "journal" => Err("journal: no journal on this host".to_string()),
            "door" => self.act(shell, "door", "release", args, "/dev/door", false),
            "lift" => self.act(shell, "lift", "answer", args, "/dev/lift", false),
            "cam" => self.act(shell, "cam", "loop", args, "/dev/cam0", false),
            "alarm" => self.act(shell, "alarm", "off", args, "/dev/alarm", true),
            "plan" => match self.block(shell.host) {
                Some(b) => access::load_plan(self.st, b)
                    .map(one)
                    .map_err(|e| format!("plan: {e}")),
                None => Err("plan: NOT CONNECTED · SSH A BUILDING FIRST".to_string()),
            },
            _ => return None,
        })
    }

    fn programs(&self) -> Vec<&'static str> {
        PROGRAMS.to_vec()
    }

    fn who(&mut self, host: HostId) -> Vec<String> {
        self.block(host)
            .map(|b| access::who_lines(self.st, b))
            .unwrap_or_default()
    }

    fn on_read(&mut self, host: HostId, path: &str, text: &str) {
        hosts::note_codes(self.st, host, path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::building_label;
    use crate::helix::hosts::{ensure_host, slug, user_for};
    use crate::helix::shell::{complete, prompt, run};
    use citygen::{BuildingKind, GenParams, Generator, NodeKind, SizePreset};

    /// A city, a residential building with people home and a server, and its district.
    fn state() -> (AppState, BuildingId, citygen::DistrictId) {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        let (b, d) = {
            let city = st.city.as_ref().unwrap();
            let world = st.world.as_ref().unwrap();
            let b = city
                .buildings
                .iter()
                .find(|b| {
                    matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
                        && world.people().iter().any(|p| p.home == Some(b.id))
                        && city
                            .net
                            .nodes
                            .iter()
                            .any(|n| n.building == Some(b.id) && n.kind == NodeKind::Server)
                })
                .unwrap();
            (b.id, b.district)
        };
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        crate::credentials::prepare(&mut st, b);
        (st, b, d)
    }

    fn state_home() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 8,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        let home = st.home.unwrap().home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        st
    }

    fn go(st: &mut AppState, shell: &mut Shell, line: &str) -> Output {
        run(&mut AppIo { st }, shell, line)
    }

    fn text(o: &Output) -> String {
        o.lines.join("\n")
    }

    fn at(st: &mut AppState, shell: &Shell) -> String {
        prompt(&AppIo { st }, shell)
    }

    #[test]
    fn devices_read_the_building_and_writes_need_a_user() {
        let (mut st, b, _) = state();
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_eq!(sh.user, "guest");
        assert_eq!(
            text(&go(&mut st, &mut sh, "cat /dev/door")),
            "locks: engaged"
        );
        let now = st.net.now(&st);
        let denied = go(&mut st, &mut sh, "echo release > /dev/door");
        assert!(denied.denied, "{denied:?}");
        assert!(text(&denied).ends_with("Permission denied"), "{denied:?}");
        assert!(!st.net.unlocked(b, now));
        // a resident's card, and the shell logs in again as the resident
        let h = st
            .interiors
            .homes_of(b)
            .unwrap()
            .households()
            .next()
            .unwrap()
            .0;
        st.character.give(crate::credentials::keycard_id(b, h), 1);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_ne!(sh.user, "guest");
        let ok = go(&mut st, &mut sh, "echo release > /dev/door");
        assert_eq!(text(&ok), "DOOR LOCKS RELEASED · 60 MIN", "{ok:?}");
        assert!(st.net.unlocked(b, now));
        assert!(text(&go(&mut st, &mut sh, "cat /dev/door")).contains("released (until "));
        let log = text(&go(&mut st, &mut sh, "cat /var/log/auth.log"));
        assert!(
            log.contains("door lock") && log.contains("spoofed"),
            "{log}"
        );
        // an unknown request is a plain error; the alarm is root's
        let bad = go(&mut st, &mut sh, "echo open sesame > /dev/door");
        assert_eq!(text(&bad), "door: unknown request");
        let alarm = go(&mut st, &mut sh, "echo off > /dev/alarm");
        assert!(alarm.denied, "{alarm:?}");
        assert_eq!(
            text(&go(&mut st, &mut sh, "cat /dev/alarm")),
            "alarm: quiet"
        );
        let net = text(&go(&mut st, &mut sh, "cat /var/log/net.log"));
        assert!(net.contains("heat 0") && net.contains("quiet"), "{net}");
        // a loop lands on the camera it names
        let cams = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .filter(|n| n.building == Some(b) && n.kind == NodeKind::Camera)
            .count();
        if cams > 0 {
            assert_eq!(text(&go(&mut st, &mut sh, "cat /dev/cam0")), "cam0: live");
            assert!(text(&go(&mut st, &mut sh, "echo loop > /dev/cam0")).contains("LOOPED"));
            assert!(text(&go(&mut st, &mut sh, "cat /dev/cam0")).starts_with("cam0: looped"));
        }
    }

    #[test]
    fn net_lists_the_district_and_ssh_moves_the_shell() {
        let (mut st, b, d) = state();
        let mut sh = new_shell(&mut st, HostId::District(d));
        let slugs: Vec<String> = crate::access::scannable(&st, d)
            .iter()
            .map(|b| hostname(&st, HostId::Building(*b)))
            .collect();
        let ls = go(&mut st, &mut sh, "ls /net");
        for s in &slugs {
            assert!(ls.lines.contains(s), "{ls:?}");
        }
        let name = hostname(&st, HostId::Building(b));
        let prefix = (1..=name.len())
            .map(|n| &name[..n])
            .find(|p| slugs.iter().filter(|s| s.starts_with(p)).count() == 1)
            .expect("a unique prefix");
        let out = go(&mut st, &mut sh, &format!("ssh {prefix}"));
        assert!(text(&out).contains("CONNECTED TO"), "{out:?}");
        assert_eq!(sh.host, HostId::Building(b));
        assert_eq!(at(&mut st, &sh), format!("guest@{name}:/$ "));
        let out = go(&mut st, &mut sh, "exit");
        assert!(!out.exit);
        assert_eq!(sh.host, HostId::District(d));
        assert!(at(&mut st, &sh).starts_with("guest@"));
        let out = go(&mut st, &mut sh, "ssh 1");
        assert!(matches!(sh.host, HostId::Building(_)), "{out:?}");
        let first = sh.host;
        assert_eq!(first, HostId::Building(crate::access::scannable(&st, d)[0]));
        go(&mut st, &mut sh, "exit");
        assert!(
            go(&mut st, &mut sh, "exit").exit,
            "exit at the origin leaves"
        );
        // errors are errors
        let nope = go(&mut st, &mut sh, "ssh zzzzzz");
        assert_eq!(nope.status, 1);
        assert!(text(&nope).contains("no such host"), "{nope:?}");
        let none = go(&mut st, &mut sh, "ssh");
        assert_eq!(none.status, 1);
        // every whole name connects, an exact name wins over the names it prefixes
        for (i, s) in slugs.iter().enumerate() {
            go(&mut st, &mut sh, &format!("ssh {s}"));
            let want = crate::access::scannable(&st, d)[i];
            assert_eq!(sh.host, HostId::Building(want), "{s}");
            go(&mut st, &mut sh, "exit");
        }
        // completion sees the programs
        let c = complete(&mut AppIo { st: &mut st }, &sh, "", "ss");
        assert_eq!(c, vec!["ssh".to_string()]);
    }

    #[test]
    fn the_rig_reads_mail_and_journal() {
        let mut st = state_home();
        let mut sh = new_shell(&mut st, HostId::Rig);
        let me = user_for(&st, HostId::Rig);
        assert_eq!(sh.user, me);
        assert_eq!(sh.home, format!("/home/{me}"));
        let inbox = text(&go(&mut st, &mut sh, "mail"));
        assert!(inbox.contains('*'), "{inbox}");
        let done = |st: &AppState| {
            st.character
                .quests
                .iter()
                .find(|q| q.quest == game::QuestId::new("first_favour"))
                .map(|q| q.done[0])
        };
        assert_eq!(done(&st), Some(false));
        let mail = text(&go(&mut st, &mut sh, "cat ~/mail/001-a-favour.txt"));
        assert!(mail.contains("Door code"), "{mail}");
        assert_eq!(done(&st), Some(true));
        assert!(st.journal.mails[0].read);
        let inbox = text(&go(&mut st, &mut sh, "mail"));
        assert!(!inbox.contains('*'), "{inbox}");
        let one = text(&go(&mut st, &mut sh, "mail 1"));
        assert!(one.starts_with("FROM: "), "{one}");
        let none = go(&mut st, &mut sh, "mail 9");
        assert_eq!(none.status, 1);
        let seed = st.city.as_ref().unwrap().seed;
        let h = st.home.unwrap().home;
        let code = format!(
            "{:04}",
            citygen::rng::door_code(seed, h.building, h.level, h.number)
        );
        let journal = text(&go(&mut st, &mut sh, "cat ~/journal.txt"));
        assert!(journal.contains(&code), "{journal}");
        assert_eq!(text(&go(&mut st, &mut sh, "journal")), journal);
        // marks never reach the screen
        assert!(!journal.contains('`'));
        let pw = text(&go(&mut st, &mut sh, "cat /etc/passwd"));
        assert!(pw.contains(&me), "{pw}");
        let v = text(&go(&mut st, &mut sh, "uname -a"));
        assert!(v.starts_with("HELIX/OS ") && v.ends_with("zenbo"), "{v}");
        // the block is on the net, and the rig's programs act on it
        assert!(text(&go(&mut st, &mut sh, "scan")).contains(&building_label(&st, h.building)));
        let out = go(&mut st, &mut sh, "alarm off");
        assert!(out.denied && text(&out).ends_with("/dev/alarm: Permission denied"));
        let now = st.net.now(&st);
        assert!(!st.net.alert(h.building, now));
        let has_lock = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .any(|n| n.building == Some(h.building) && n.kind == NodeKind::DoorLock);
        if has_lock {
            let out = go(&mut st, &mut sh, "door release");
            assert_eq!(text(&out), "DOOR LOCKS RELEASED · 60 MIN");
            assert!(st.net.unlocked(h.building, now));
        }
        // mail is the rig's alone
        let ssh = go(&mut st, &mut sh, "ssh 1");
        assert!(matches!(sh.host, HostId::Building(_)), "{ssh:?}");
        assert_eq!(go(&mut st, &mut sh, "mail").status, 1);
    }

    #[test]
    fn programs_keep_the_old_voice() {
        let (mut st, b, d) = state();
        let mut sh = new_shell(&mut st, HostId::District(d));
        let nobody = go(&mut st, &mut sh, "door release");
        assert_eq!(nobody.status, 1);
        assert!(text(&nobody).contains("NOT CONNECTED"), "{nobody:?}");
        let scan = text(&go(&mut st, &mut sh, "scan"));
        let name = building_label(&st, b);
        let row = scan.lines().find(|l| l.contains(&name)).expect("a row");
        assert!(row.contains("SEC ") && row.contains("ICE ") && row.contains("GUEST"));
        let n: usize = row.split_whitespace().next().unwrap().parse().unwrap();
        go(&mut st, &mut sh, &format!("ssh {n}"));
        assert_eq!(sh.host, HostId::Building(b));
        // scan still shows the district from inside a building
        assert_eq!(text(&go(&mut st, &mut sh, "scan")), scan);
        let out = go(&mut st, &mut sh, "door release");
        assert!(out.denied, "{out:?}");
        assert_eq!(text(&out), "door: /dev/door: Permission denied");
        for cmd in ["lift answer", "cam loop"] {
            let out = go(&mut st, &mut sh, cmd);
            assert!(out.denied, "{cmd}: {out:?}");
        }
        // the denial is not a pipe's to carry
        let piped = go(&mut st, &mut sh, "door release | wc");
        assert!(text(&piped).contains("Permission denied"));
        assert!(text(&go(&mut st, &mut sh, "plan")).contains("GROUND FLOOR PLAN LOADED"));
        let place = citygen::Place::Floor {
            building: b,
            level: 0,
        };
        let vis = st.player.as_ref().unwrap().vis_at(place).expect("a plan");
        assert!(
            vis.iter()
                .any(|(_, v)| *v == crate::walker::Visibility::Remembered)
        );
        let world = st.world.as_ref().unwrap();
        let home = world
            .inside(b)
            .into_iter()
            .find(|o| o.role == sim::OccupantRole::Living)
            .expect("a resident home at ten at night");
        let who = slug(&world.people()[home.id.0 as usize].name);
        let who_out = text(&go(&mut st, &mut sh, "who"));
        assert!(
            who_out.contains(&who) && who_out.contains("at home"),
            "{who_out}"
        );
        // a resident's hand: the doors, the lifts, the cameras, as the old verbs did
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        sh.origin = HostId::District(d);
        let now = st.net.now(&st);
        assert!(text(&go(&mut st, &mut sh, "door release")).contains("DOOR LOCKS RELEASED"));
        assert!(st.net.unlocked(b, now));
        let lift = text(&go(&mut st, &mut sh, "lift answer"));
        let has_lift = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .any(|n| n.building == Some(b) && n.kind == NodeKind::LiftCtrl);
        assert_eq!(lift.contains("LIFTS ANSWERING"), has_lift, "{lift}");
        let usage = go(&mut st, &mut sh, "door");
        assert_eq!(usage.status, 1);
        // the alarm is root's: a backdoor makes the shell root
        let alarm = go(&mut st, &mut sh, "alarm off");
        assert!(alarm.denied, "{alarm:?}");
        let server = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .find(|n| n.building == Some(b) && n.kind == NodeKind::Server)
            .map(|n| n.id)
            .unwrap();
        st.net.land(
            b,
            game::HackTarget::Node(server),
            game::Target::Node(NodeKind::Server),
            game::Op::Backdoor,
            now,
        );
        st.backdoors.plant_net(b, 1, 0, now);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_eq!(sh.user, "root");
        let quiet = go(&mut st, &mut sh, "alarm off");
        assert_eq!(text(&quiet), "alarm: NO ALARM");
        assert_eq!(quiet.status, 1);
        st.net.trace(b).alert_until = Some(now + 30);
        // in lockdown the locks hold and the program says so
        let held = go(&mut st, &mut sh, "door release");
        assert!(text(&held).contains("LOCKDOWN IN FORCE"), "{held:?}");
        assert!(!text(&held).contains("60 MIN"));
        assert_eq!(text(&go(&mut st, &mut sh, "cat /dev/door")), "lockdown");
        assert_eq!(
            text(&go(&mut st, &mut sh, "cat /dev/alarm")),
            "alarm: RINGING"
        );
        let net = text(&go(&mut st, &mut sh, "cat /var/log/net.log"));
        assert!(net.contains("alert until"), "{net}");
        assert_eq!(text(&go(&mut st, &mut sh, "alarm off")), "ALARM CLEARED");
        assert!(!st.net.alert(b, now));
    }

    /// A resident with a code in their notes: (building, user, home).
    fn noted(st: &mut AppState) -> (BuildingId, String, String) {
        let ids: Vec<BuildingId> = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .filter(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock))
            .map(|b| b.id)
            .collect();
        for b in ids {
            let h = ensure_host(st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let Ok(id) = h.fs.resolve("/", "/", &format!("{}/notes.txt", u.home)) else {
                    continue;
                };
                if let Kind::File(t) = &h.fs.node(id).kind
                    && t.contains("door is")
                {
                    return (b, u.name.clone(), u.home.clone());
                }
            }
        }
        panic!("a notes file with a code")
    }

    #[test]
    fn every_reader_learns_a_code_not_only_cat() {
        for line in [
            "grep door ~/notes.txt",
            "head -n 1 ~/notes.txt",
            "tail -n 1 ~/notes.txt",
            "wc ~/notes.txt",
        ] {
            let (mut st, ..) = state();
            let (b, user, home) = noted(&mut st);
            let mut sh = Shell::new(HostId::Building(b), &user, &home);
            go(&mut st, &mut sh, line);
            assert!(!st.journal.codes().is_empty(), "{line} learned nothing");
            let n = st.journal.codes().len();
            go(&mut st, &mut sh, line);
            assert_eq!(st.journal.codes().len(), n, "{line}: learned once");
        }
    }

    #[test]
    fn a_file_of_every_code_written_by_hand_teaches_nothing() {
        let (mut st, ..) = state();
        let (b, user, home) = noted(&mut st);
        let seed = st.city.as_ref().unwrap().seed;
        let every: Vec<String> = st
            .interiors
            .homes_of(b)
            .unwrap()
            .flats()
            .map(|(l, n)| format!("{:04}", citygen::rng::door_code(seed, b, l, n)))
            .collect();
        let mut sh = Shell::new(HostId::Building(b), &user, &home);
        go(
            &mut st,
            &mut sh,
            &format!("echo door code {} > /tmp/g", every.join(" ")),
        );
        let out = go(&mut st, &mut sh, "cat /tmp/g");
        assert!(text(&out).contains(&every[0]), "{out:?}");
        assert!(
            st.journal.codes().is_empty(),
            "guessed: {:?}",
            st.journal.codes()
        );
        go(&mut st, &mut sh, "cat ~/notes.txt");
        assert!(!st.journal.codes().is_empty());
    }

    #[test]
    fn twin_names_each_ssh_to_their_own_building() {
        let (mut st, ..) = state();
        let (d, a, b) = crate::helix::hosts::twins(&mut st, "Twin Tower");
        let mut sh = new_shell(&mut st, HostId::District(d));
        let ls = go(&mut st, &mut sh, "ls /net");
        let second = format!("twin-tower-{}", b.index());
        assert!(
            ls.lines.iter().any(|l| l == "twin-tower") && ls.lines.contains(&second),
            "{ls:?}"
        );
        for (name, want) in [("twin-tower".to_string(), a), (second, b)] {
            let out = go(&mut st, &mut sh, &format!("ssh {name}"));
            assert!(text(&out).contains("CONNECTED TO"), "{name}: {out:?}");
            assert_eq!(sh.host, HostId::Building(want), "ssh {name}");
            assert_eq!(at(&mut st, &sh), format!("guest@{name}:/$ "));
            go(&mut st, &mut sh, "exit");
        }
    }

    #[test]
    fn notes_learn_codes_through_cat() {
        let (mut st, ..) = state();
        let mut found = None;
        let ids: Vec<BuildingId> = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .filter(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock))
            .map(|b| b.id)
            .collect();
        'all: for b in ids {
            let h = ensure_host(&mut st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let path = format!("{}/notes.txt", u.home);
                let Ok(id) = h.fs.resolve("/", "/", &path) else {
                    continue;
                };
                if let Kind::File(t) = &h.fs.node(id).kind
                    && t.contains("door is")
                {
                    found = Some((b, u.name.clone(), u.home.clone()));
                    break 'all;
                }
            }
        }
        let (b, user, home) = found.expect("a notes file with a code");
        let mut sh = Shell::new(HostId::Building(b), &user, &home);
        assert!(st.journal.codes().is_empty());
        let before = st.log.iter().filter(|e| e.text.contains("notes")).count();
        let out = go(&mut st, &mut sh, "cat ~/notes.txt");
        assert!(text(&out).contains("door is"));
        let n = st.journal.codes().len();
        assert!(n >= 1);
        go(&mut st, &mut sh, "cat ~/notes.txt");
        assert_eq!(st.journal.codes().len(), n, "learned once");
        assert_eq!(
            st.log.iter().filter(|e| e.text.contains("notes")).count(),
            before + n
        );
        // a guest cannot read it, so learns nothing
        let mut guest = Shell::new(HostId::Building(b), "guest", "/tmp");
        let denied = go(&mut st, &mut guest, &format!("cat {home}/notes.txt"));
        assert!(denied.denied, "{denied:?}");
        assert_eq!(st.journal.codes().len(), n);
    }
}
