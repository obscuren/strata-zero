//! The world behind HELIX/OS: the building as `/dev`, the district as `/net`, the
//! old terminal verbs as programs. `AppIo` is the `Io` the game gives the shell.

use super::hosts::{self, ensure_host, hostname, user_for};
use super::shell::{HostId, Io, Output, Shell};
use super::vfs::{Dev, GUEST, Mode, ROOT, Vfs};
use crate::access::{self, Tier, building_label, tier_on};
use crate::app::AppState;
use crate::ui::device::Look;
use citygen::BuildingId;

use super::vfs::Kind;

/// The shell's window on the game: every device and program reads and writes `st`.
pub struct AppIo<'a> {
    pub st: &'a mut AppState,
}

/// The programs the world adds to the builtins.
const PROGRAMS: &[&str] = &[
    "scan", "ssh", "connect", "mail", "journal", "door", "lift", "cam", "alarm", "plan",
    "backdoor", "burn", "route",
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
    st.backdoors.trap_login = None;
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

fn key_path(user: &str) -> String {
    if user == ROOT {
        "/root/.ssh/authorized_keys".to_string()
    } else {
        format!("/home/{user}/.ssh/authorized_keys")
    }
}

fn key_line(st: &AppState) -> String {
    let seed = st.city.as_ref().map_or(0, |c| c.seed);
    let me = hosts::rig_user(st);
    format!(
        "helix-ed25519 {:016x} {me}@zenbo",
        citygen::rng::hash64(seed, 0x6B65)
    )
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
            HostId::Rig => {
                let mut list: Vec<BuildingId> = self.block(host).into_iter().collect();
                for d in self.st.backdoors.standing() {
                    if !list.contains(&d.building) {
                        list.push(d.building);
                    }
                }
                list
            }
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
                // A site's host sits behind its firewall: from outside its Net, refused
                // while a lock is whole, before a key is tried (Ruling 15 of the plan
                // `docs/superpowers/plans/2026-11-11-site-net.md`). A shell on one of its
                // own terminals is on the host already.
                if crate::sitenet::is_site(self.st, b) && !crate::sitenet::firewall_open(self.st, b)
                {
                    let n = crate::sitenet::broken(self.st, b).len();
                    return Err(format!(
                        "ssh: {}: FIREWALL · {n} LOCK{}",
                        hostname(self.st, HostId::Building(b)),
                        if n == 1 { "" } else { "S" }
                    ));
                }
                let via = self.st.backdoors.held(b).is_some();
                let mut audit = false;
                let mut key_user = None;
                if via {
                    let now = self.st.net.now(self.st);
                    let seed = self.st.city.as_ref().map_or(0, |c| c.seed);
                    let security = access::security_of(self.st, b);
                    let who = self.st.backdoors.held(b).map(|d| d.user_label());
                    key_user = who.clone();
                    match self.st.backdoors.check(b, security, seed, now) {
                        crate::backdoors::Use::Refused => {
                            return Err(format!(
                                "ssh: {}: connection refused · key rejected",
                                hostname(self.st, HostId::Building(b))
                            ));
                        }
                        crate::backdoors::Use::Trap => {
                            audit = true;
                            self.st.backdoors.trap_login = who.map(|u| (b, u));
                            crate::net::lock_down(self.st, b);
                            let _ = self.st.net.add_heat(
                                b,
                                crate::backdoors::HONEYPOT_NOISE,
                                0,
                                now,
                                false,
                            );
                            if shell.origin == HostId::Rig {
                                self.st.backdoors.start_route(None);
                                self.st.backdoors.start_trace(now);
                                self.st.backdoors.end_route();
                                self.st.warn("honeypot · they were waiting · trace running");
                            }
                        }
                        crate::backdoors::Use::Works => {
                            self.st.backdoors.spend(b, crate::backdoors::COST_SSH);
                        }
                    }
                }
                login(self.st, shell, HostId::Building(b));
                if via {
                    let now = self.st.net.now(self.st);
                    let user = key_user.unwrap_or_else(|| shell.user.clone());
                    self.st.backdoors.logins.push((now, b, user));
                }
                let mut lines = vec![format!(
                    "CONNECTED TO {} · CLEARANCE {}{}",
                    building_label(self.st, b),
                    tier_on(self.st, b).label(),
                    if via { " · via your backdoor" } else { "" }
                )];
                if audit {
                    lines.push("AUDIT · THIS SESSION IS BEING RECORDED".to_string());
                }
                Ok(Output {
                    lines,
                    ..Output::default()
                })
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

    /// `burn <line>`: cuts one of the rig's lines, by number, name or prefix.
    fn burn(&mut self, shell: &Shell, args: &[String]) -> Result<Output, String> {
        if shell.host != HostId::Rig {
            return Err("burn: rig only".to_string());
        }
        let [what] = args else {
            return Err("usage: burn <host>".to_string());
        };
        let what = what.to_lowercase();
        // only the lines you hold: the home block is no backdoor unless it holds one
        let all = self.net_of(HostId::Rig);
        let held: Vec<(BuildingId, String)> = all
            .iter()
            .filter(|b| self.st.backdoors.held(**b).is_some())
            .map(|b| (*b, hostname(self.st, HostId::Building(*b))))
            .collect();
        let pick: Vec<&(BuildingId, String)> = if let Ok(n) = what.parse::<usize>() {
            n.checked_sub(1)
                .and_then(|i| all.get(i))
                .and_then(|b| held.iter().find(|(x, _)| x == b))
                .into_iter()
                .collect()
        } else if let Some(h) = held.iter().find(|(_, n)| *n == what) {
            vec![h]
        } else {
            held.iter().filter(|(_, n)| n.starts_with(&what)).collect()
        };
        match pick.as_slice() {
            [] => Err(format!("burn: {what}: no such backdoor")),
            [(b, name)] => {
                let now = self.st.net.now(self.st);
                let r = self.st.backdoors.burn(*b, crate::backdoors::Why::You, now);
                let mut line = format!("BURNED · {name}");
                if r.cut_trace {
                    line.push_str(" · TRACE CUT");
                    // the building sees the line go: it alarms as a console alarm does
                    crate::net::lock_down(self.st, *b);
                }
                Ok(one(line))
            }
            many => Err(format!(
                "burn: {what}: ambiguous: {}",
                many.iter()
                    .map(|(_, n)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// A device write through a backdoor spends some of its life.
    fn spend_write(&mut self, host: HostId, user: &str) {
        let HostId::Building(b) = host else { return };
        if self
            .st
            .backdoors
            .live(b)
            .is_some_and(|d| d.user_label() == user)
        {
            self.st.backdoors.spend(b, crate::backdoors::COST_WRITE);
        }
    }

    fn backdoor(&mut self, shell: &Shell) -> Result<Output, String> {
        let HostId::Building(b) = shell.host else {
            return Err("backdoor: no server here".to_string());
        };
        if shell.user == GUEST {
            return Ok(Output {
                lines: vec!["backdoor: Permission denied".into()],
                status: 1,
                denied: true,
                ..Default::default()
            });
        }
        let now = self.st.net.now(self.st);
        if let Err(r) = self.st.backdoors.can_plant(b, now) {
            return Err(format!("backdoor: {}", r.words()));
        }
        let path = key_path(&shell.user);
        let line = key_line(self.st);
        let security = access::security_of(self.st, b);
        let seed = self.st.city.as_ref().map_or(0, |c| c.seed);
        {
            let fs = self.fs(shell.host);
            let dir = path
                .rsplit_once('/')
                .map(|(d, _)| d.to_string())
                .unwrap_or_default();
            if fs.resolve("/", "/", &dir).is_err() {
                fs.mkdir_as(&dir, &shell.user, Mode::RW_NONE);
            }
            let existing = match fs.read(ROOT, "/", "/", &path) {
                Ok(Kind::File(t)) => t.clone(),
                _ => String::new(),
            };
            fs.put(
                &path,
                &format!("{existing}{line}\n"),
                &shell.user,
                Mode::RW_NONE,
            );
        }
        self.st
            .backdoors
            .plant_key(b, &shell.user, security, seed, now);
        crate::jobs::send(self.st, crate::jobs::Event::Planted { building: b });
        let (event, _) = self
            .st
            .net
            .add_heat(b, crate::backdoors::PLANT_NOISE, 0, now, false);
        if event == game::TraceEvent::Alarm {
            let line = crate::net::raise_alarm(self.st, b);
            self.st.warn(line);
        }
        Ok(one(format!(
            "KEY PLANTED · {}@{} · the rig holds {} of {}",
            shell.user,
            hostname(self.st, shell.host),
            self.st.backdoors.slots_used(),
            crate::backdoors::SLOTS
        )))
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
            [] => Ok(Output {
                open_mail: true,
                ..Output::default()
            }),
            [n] => {
                let i = n
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .ok_or_else(|| "usage: mail [n]".to_string())?;
                let out = access::mail_open(self.st, i).map_err(|e| format!("mail: {e}"))?;
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
        if done.is_ok() {
            self.spend_write(shell.host, &shell.user);
        }
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
                let mut rows = vec![access::scan_row(self.st, i, *b)];
                if host == HostId::Rig {
                    rows.extend(access::line_row(self.st, *b));
                }
                return Ok(rows);
            }
            Dev::NetLog if host == HostId::Rig => return Ok(access::rig_net_lines(self.st)),
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
        user: &str,
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
        if done.is_ok() {
            self.spend_write(host, user);
        }
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
            "backdoor" => self.backdoor(shell),
            "burn" => self.burn(shell, args),
            "route" if shell.host == HostId::Rig => Ok(lines(access::rig_net_lines(self.st))),
            "route" => Err("route: rig only".to_string()),
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

    fn changed(&mut self, host: HostId, path: &str) {
        let HostId::Building(b) = host else { return };
        let Some(d) = self.st.backdoors.held(b) else {
            return;
        };
        let crate::backdoors::Kind::Key { user } = &d.kind else {
            return;
        };
        let key = key_path(user);
        let dir = key.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let under = |p: &str| key == p || key.starts_with(&format!("{p}/"));
        if path != dir && !under(path) {
            return;
        }
        // look at the file: the key may well still be there
        let line = key_line(self.st);
        let kept = matches!(
            ensure_host(self.st, host).fs.read("root", "/", "/", &key),
            Ok(Kind::File(text)) if text.lines().any(|l| l == line)
        );
        if !kept {
            let now = self.st.net.now(self.st);
            self.st.backdoors.burn(b, crate::backdoors::Why::You, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::building_label;
    use crate::helix::hosts::{ensure_host, slug, user_for};
    use crate::helix::shell::{complete, prompt, run};
    use crate::sitenet::testutil::{crack_server, shell_on, street_shell_listing};
    use citygen::{BuildingKind, GenParams, Generator, NodeKind, SizePreset};
    use std::collections::BTreeSet;

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
        // The welcome favour's mail, for the inbox to read.
        crate::jobs::first_favour(&mut st);
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
    fn a_cracked_server_lets_the_shell_plant_as_root() {
        let (mut st, b, _) = state();
        let now = st.net.now(&st);
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
            game::Op::Crack,
            now,
        );
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_eq!(sh.user, "root");
        let out = text(&go(&mut st, &mut sh, "backdoor"));
        assert!(out.starts_with("KEY PLANTED · root@"), "{out}");
        assert!(
            text(&go(&mut st, &mut sh, "cat /root/.ssh/authorized_keys"))
                .starts_with("helix-ed25519 ")
        );
        assert!(matches!(
            st.backdoors.held(b).map(|d| d.kind.clone()),
            Some(crate::backdoors::Kind::Key { user }) if user == "root"
        ));
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
    fn mail_n_prints_a_head_block() {
        let mut st = state_home();
        let mut sh = new_shell(&mut st, HostId::Rig);
        st.journal.mails.clear();
        let mail = |from: &str, subject: &str| crate::credentials::Mail {
            from: from.to_string(),
            subject: subject.to_string(),
            body: vec!["one line".to_string(), String::new(), "two".to_string()],
            read: false,
            at: 600,
            job: None,
            story: None,
        };
        let a = mail("Mezz", "the news");
        let time = access::mail_time(&a);
        assert_eq!(time.len(), 5);
        st.journal.mails.push(a);
        let rule = "─".repeat(44);
        let first = text(&go(&mut st, &mut sh, "mail 1"));
        let want = format!(
            "MAIL 1 OF 1 · NEW\nFROM     MEZZ\nSUBJECT  THE NEWS\nTIME     {time}\n{rule}\none line\n\ntwo\n{rule}\nNO NEW MAIL"
        );
        assert_eq!(first, want);
        let again = text(&go(&mut st, &mut sh, "mail 1"));
        assert!(again.starts_with("MAIL 1 OF 1\n"), "{again}");
        st.journal.mails.push(mail("Seren", "a call"));
        st.journal.mails.push(mail("Odile", "a tab"));
        let left = text(&go(&mut st, &mut sh, "mail 2"));
        assert!(left.starts_with("MAIL 2 OF 3 · NEW\n"), "{left}");
        assert!(left.ends_with("\n1 NEW LEFT · mail"), "{left}");
    }

    #[test]
    fn the_rig_reads_mail_and_journal() {
        let mut st = state_home();
        let mut sh = new_shell(&mut st, HostId::Rig);
        let me = user_for(&st, HostId::Rig);
        assert_eq!(sh.user, me);
        assert_eq!(sh.home, format!("/home/{me}"));
        // `mail` asks the screen for HELIX mail, and wins the line as nano does
        let out = go(&mut st, &mut sh, "mail");
        assert!(out.open_mail && out.lines.is_empty(), "{out:?}");
        assert!(!st.journal.mails[0].read, "opening reads nothing");
        let out = go(&mut st, &mut sh, "mail; echo hi");
        assert!(out.open_mail && out.lines.is_empty(), "{out:?}");
        let out = go(&mut st, &mut sh, "echo hi | mail");
        assert!(out.open_mail, "{out:?}");
        let mail = text(&go(&mut st, &mut sh, "cat ~/mail/001-a-favour.txt"));
        assert!(mail.contains("Door code"), "{mail}");
        assert!(mail.starts_with("MAIL 1 OF 1 · NEW\nFROM     "), "{mail}");
        assert!(st.journal.mails[0].read);
        let one = text(&go(&mut st, &mut sh, "mail 1"));
        assert!(one.starts_with("MAIL 1 OF 1\nFROM     "), "{one}");
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

    #[test]
    fn backdoor_plants_a_key_as_a_resident_and_refuses_in_order() {
        let (mut st, b, d) = state();
        // guest: denied
        let mut sh = new_shell(&mut st, HostId::Building(b));
        let out = go(&mut st, &mut sh, "backdoor");
        assert!(
            out.denied && text(&out) == "backdoor: Permission denied",
            "{out:?}"
        );
        // a district host has no server
        let mut dh = new_shell(&mut st, HostId::District(d));
        assert_eq!(
            text(&go(&mut st, &mut dh, "backdoor")),
            "backdoor: no server here"
        );
        // a resident plants
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        let me = sh.user.clone();
        let out = go(&mut st, &mut sh, "backdoor");
        assert_eq!(
            text(&out),
            format!(
                "KEY PLANTED · {me}@{} · the rig holds 1 of 3",
                hostname(&st, HostId::Building(b))
            )
        );
        let key = text(&go(&mut st, &mut sh, "cat ~/.ssh/authorized_keys"));
        assert!(
            key.starts_with("helix-ed25519 ") && key.ends_with("@zenbo"),
            "{key}"
        );
        let ls = text(&go(&mut st, &mut sh, "ls -l ~/.ssh"));
        assert!(
            ls.lines()
                .any(|l| l.contains("authorized_keys") && l.starts_with(&format!("-rw---- {me}"))),
            "{ls}"
        );
        assert!(st.backdoors.held(b).is_some());
        assert!(
            st.net
                .traces
                .get(&b)
                .is_some_and(|t| t.heat >= crate::backdoors::PLANT_NOISE)
        );
        // again: already here
        assert_eq!(
            text(&go(&mut st, &mut sh, "backdoor")),
            "backdoor: already planted here"
        );
        // fill the rig, then the cap
        st.backdoors.plant_net(BuildingId(9000), 1, 0, 0);
        st.backdoors.plant_net(BuildingId(9001), 1, 0, 0);
        st.backdoors.plant_net(BuildingId(9002), 1, 0, 0);
        st.backdoors.burn(b, crate::backdoors::Why::Decay, 0);
        let out = go(&mut st, &mut sh, "backdoor");
        assert_eq!(
            text(&out),
            "backdoor: the rig holds 3 lines · burn one first"
        );
        assert_eq!(out.status, 1);
        // patched beats full; seized beats patched
        st.backdoors
            .burn(BuildingId(9000), crate::backdoors::Why::You, 0);
        st.backdoors.patched.insert(b, 10_000);
        assert_eq!(
            text(&go(&mut st, &mut sh, "backdoor")),
            "backdoor: hole patched · try again tomorrow"
        );
        st.backdoors.rig_seized_until = Some(10_000);
        assert_eq!(
            text(&go(&mut st, &mut sh, "backdoor")),
            "backdoor: the rig is seized · no line home"
        );
        // man knows it
        assert!(text(&go(&mut st, &mut sh, "man backdoor")).starts_with("backdoor - "));
    }

    #[test]
    fn the_rig_lists_lines_and_burns_them() {
        let mut st = state_home();
        let home = st.home.unwrap().home.building;
        let seed = st.city.as_ref().unwrap().seed;
        // a backdoor on some other scannable building
        let other = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .find(|x| x.id != home && x.kind.is_closed())
            .map(|x| x.id)
            .unwrap();
        st.backdoors.plant_net(other, 1, seed, 0);
        let mut sh = new_shell(&mut st, HostId::Rig);
        let ls = text(&go(&mut st, &mut sh, "ls /net"));
        let other_name = hostname(&st, HostId::Building(other));
        assert!(ls.contains(&other_name), "{ls}");
        let cat = text(&go(&mut st, &mut sh, &format!("cat /net/{other_name}")));
        assert!(cat.contains("LINE  root  STABLE  planted day"), "{cat}");
        let route = text(&go(&mut st, &mut sh, "route"));
        assert!(
            route.contains("NO ROUTE · you are home")
                && route.contains("TRACE  none")
                && route.contains("LINES  1 of 3"),
            "{route}"
        );
        let log = text(&go(&mut st, &mut sh, "cat /var/log/net.log"));
        assert_eq!(log, route);
        // burn by prefix
        let out = text(&go(&mut st, &mut sh, &format!("burn {}", &other_name[..3])));
        assert_eq!(out, format!("BURNED · {other_name}"));
        assert!(st.backdoors.held(other).is_none());
        let ls = text(&go(&mut st, &mut sh, "ls /net"));
        assert!(!ls.contains(&other_name), "{ls}");
        let bad = go(&mut st, &mut sh, "burn nope");
        assert_eq!(
            (text(&bad).as_str(), bad.status),
            ("burn: nope: no such backdoor", 1)
        );
        // a trace on the relay: burning cuts it and the building alarms
        st.backdoors.plant_net(other, 1, seed, 5_000);
        st.backdoors.patched.clear();
        st.backdoors.start_route(Some(other));
        st.backdoors.start_trace(5_000);
        st.backdoors.end_route();
        let route = text(&go(&mut st, &mut sh, "route"));
        assert!(route.contains(&format!("TRACE ▸ {other_name}")), "{route}");
        let out = text(&go(&mut st, &mut sh, &format!("burn {other_name}")));
        assert_eq!(out, format!("BURNED · {other_name} · TRACE CUT"));
        let now = st.net.now(&st);
        assert!(st.net.alert(other, now));
        assert!(st.backdoors.trace.is_none());
        // not on a building host
        let mut bh = new_shell(&mut st, HostId::Building(home));
        assert_eq!(go(&mut st, &mut bh, "route").status, 1);
    }

    #[test]
    fn rm_of_the_key_file_burns_the_key_once() {
        let (mut st, b, _) = state();
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        go(&mut st, &mut sh, "backdoor");
        assert!(st.backdoors.held(b).is_some());
        assert_eq!(go(&mut st, &mut sh, "rm ~/.ssh/authorized_keys").status, 0);
        assert!(st.backdoors.held(b).is_none());
        assert!(matches!(
            st.backdoors.list[0].state,
            crate::backdoors::State::Burned {
                why: crate::backdoors::Why::You,
                ..
            }
        ));
        // the directory may go too, and nothing double-burns
        assert_eq!(go(&mut st, &mut sh, "rm -r ~/.ssh").status, 0);
        assert_eq!(st.backdoors.list.len(), 1);
        // patched: planting again is refused
        assert_eq!(
            text(&go(&mut st, &mut sh, "backdoor")),
            "backdoor: hole patched · try again tomorrow"
        );
    }

    #[test]
    fn moving_or_overwriting_the_key_file_burns_the_key_too() {
        let planted = || {
            let (mut st, b, _) = state();
            crate::credentials::copy_codes(&mut st, b, 1);
            let mut sh = new_shell(&mut st, HostId::Building(b));
            go(&mut st, &mut sh, "backdoor");
            assert!(st.backdoors.held(b).is_some());
            (st, b, sh)
        };
        let (mut st, b, mut sh) = planted();
        assert_eq!(
            go(&mut st, &mut sh, "mv ~/.ssh/authorized_keys ~/x").status,
            0
        );
        assert!(st.backdoors.held(b).is_none());
        let (mut st, b, mut sh) = planted();
        assert_eq!(
            go(&mut st, &mut sh, "echo nothing > ~/.ssh/authorized_keys").status,
            0
        );
        assert!(st.backdoors.held(b).is_none());
        // an append keeps the line, so the key stands
        let (mut st, b, mut sh) = planted();
        assert_eq!(
            go(&mut st, &mut sh, "echo note >> ~/.ssh/authorized_keys").status,
            0
        );
        assert!(st.backdoors.held(b).is_some());
        // a copy of the key file elsewhere touches nothing
        assert_eq!(
            go(&mut st, &mut sh, "cp ~/.ssh/authorized_keys ~/y").status,
            0
        );
        assert!(st.backdoors.held(b).is_some());
    }

    #[test]
    fn ssh_rides_a_key_from_the_rig_and_is_refused_once_it_burns() {
        let mut st = state_home();
        let home = st.home.unwrap().home.building;
        // a key on the home block as a resident other than you: use copy_codes then plant as that user
        crate::credentials::copy_codes(&mut st, home, 1);
        let user = crate::helix::hosts::user_for(&st, HostId::Building(home));
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_key(home, &user, 100, seed, 0);
        // forget the codes, so only the key gets us in
        st.journal = Default::default();
        crate::credentials::move_in(&mut st);
        let mut sh = new_shell(&mut st, HostId::Rig);
        let out = text(&go(&mut st, &mut sh, "ssh 1"));
        assert!(out.ends_with("· via your backdoor"), "{out}");
        assert_eq!(sh.user, user);
        assert_eq!(
            st.backdoors.held(home).unwrap().spent,
            crate::backdoors::COST_SSH
        );
        let log = text(&go(&mut st, &mut sh, "cat /var/log/auth.log"));
        assert!(log.contains(&format!("login  {user} via key")), "{log}");
        go(&mut st, &mut sh, "exit");
        // a device write through the key costs too
        go(&mut st, &mut sh, "ssh 1");
        let before = st.backdoors.held(home).unwrap().spent;
        go(&mut st, &mut sh, "door release");
        assert_eq!(
            st.backdoors.held(home).unwrap().spent,
            before + crate::backdoors::COST_WRITE
        );
        go(&mut st, &mut sh, "exit");
        // burned by time: refused, and the journal knows
        let d = st
            .backdoors
            .list
            .iter_mut()
            .find(|d| d.building == home)
            .unwrap();
        d.spent = d.nominal * 2;
        let out = go(&mut st, &mut sh, "ssh 1");
        assert_eq!(out.status, 1);
        assert_eq!(
            text(&out),
            format!(
                "ssh: {}: connection refused · key rejected",
                hostname(&st, HostId::Building(home))
            )
        );
        assert_eq!(sh.host, HostId::Rig);
        assert_eq!(
            st.backdoors.list[0].health(0),
            crate::backdoors::Health::Burned
        );
    }

    #[test]
    fn ssh_to_a_site_from_the_street_is_refused_while_its_firewall_stands() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let (mut sh, n) = street_shell_listing(&mut st, b); // n: the site's row in `net`
        let out = text(&go(&mut st, &mut sh, &format!("ssh {n}")));
        let host = hostname(&st, HostId::Building(b));
        assert_eq!(out, format!("ssh: {host}: FIREWALL · 3 LOCKS"));
        assert_ne!(sh.host, HostId::Building(b));
        let now = st.net.now(&st);
        for i in 0..3 {
            st.net.break_lock(b, i, now);
        }
        go(&mut st, &mut sh, &format!("ssh {n}"));
        assert_eq!(sh.host, HostId::Building(b));
    }

    #[test]
    fn a_sites_firewall_refuses_ssh_from_the_rig_before_the_key_is_spent() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let seed = st.city.as_ref().unwrap().seed;
        let now = st.net.now(&st);
        st.backdoors.plant_net(b, 99, seed, now);
        let mut sh = new_shell(&mut st, HostId::Rig);
        let host = hostname(&st, HostId::Building(b));
        let out = go(&mut st, &mut sh, &format!("ssh {host}"));
        assert_eq!(text(&out), format!("ssh: {host}: FIREWALL · 3 LOCKS"));
        assert_eq!(out.status, 1);
        assert_eq!(sh.host, HostId::Rig);
        assert_eq!(st.backdoors.held(b).unwrap().spent, 0, "the key unspent");
        assert!(st.backdoors.logins.is_empty());
    }

    #[test]
    fn a_building_wide_command_needs_root_on_a_site() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let mut sh = shell_on(&mut st, b); // logged in on b's host as a user (a held terminal)
        let out = text(&go(&mut st, &mut sh, "door release"));
        assert!(out.contains("ROOT ONLY"), "{out}");
        assert_eq!(out, "door: ROOT ONLY · CRACK THE SERVER");
        for line in ["lift answer", "cam loop", "echo release > /dev/door"] {
            let out = text(&go(&mut st, &mut sh, line));
            assert!(
                out.ends_with(": ROOT ONLY · CRACK THE SERVER"),
                "{line}: {out}"
            );
        }
        assert!(!st.net.hacks.iter().any(|h| h.op == game::Op::Spoof));
        crack_server(&mut st, b);
        let out = text(&go(&mut st, &mut sh, "door release"));
        let net = crate::sitenet::net_of(&st, b).unwrap();
        let locks = |n: &&citygen::sitenet::SiteNodeInfo| {
            matches!(n.kind, NodeKind::DoorLock | NodeKind::GateSwitch)
        };
        let k = (net.nodes.iter().filter(locks))
            .filter(|n| net.on_riser(n.node.level))
            .count();
        assert_eq!(out, format!("{k} DOOR LOCKS RELEASED · 60 MIN"));
        assert!(
            net.nodes
                .iter()
                .filter(locks)
                .any(|n| !net.on_riser(n.node.level))
        );
        for n in net.nodes.iter().filter(locks) {
            assert_eq!(
                crate::sitenet::hacked(&st, n.node, &[game::Op::Spoof]),
                net.on_riser(n.node.level),
                "{}",
                n.name
            );
        }
        assert_eq!(
            text(&go(&mut st, &mut sh, "cat /dev/door")),
            format!("locks: {k}/{k} released")
        );
    }

    #[test]
    fn man_says_what_a_sites_host_wants() {
        let (mut st, _, d) = state();
        let mut sh = new_shell(&mut st, HostId::District(d));
        let page = |st: &mut AppState, sh: &mut Shell, cmd: &str| {
            go(st, sh, &format!("man {cmd}")).lines.join(" ")
        };
        for cmd in ["door", "lift", "cam", "alarm"] {
            let man = page(&mut st, &mut sh, cmd);
            assert!(
                man.contains("On a hotel's or an office's host it wants root"),
                "{cmd}: {man}"
            );
        }
        let ssh = page(&mut st, &mut sh, "ssh");
        assert!(ssh.contains("while its firewall stands"), "{ssh}");
    }

    /// The `cam*` devices `ls /dev` lists on the shell's host.
    fn cam_devices(st: &mut AppState, sh: &mut Shell) -> BTreeSet<String> {
        let ls = go(st, sh, "ls /dev");
        (ls.lines.into_iter())
            .filter(|l| l.starts_with("cam"))
            .collect()
    }

    #[test]
    fn a_sites_camera_devices_follow_the_cameras_on_its_riser() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let copy = st.interiors.copy(b).unwrap().clone();
        // The host first opened while the plain office stands: no cameras of its own.
        crate::venues::end(&mut st, b).unwrap();
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_eq!(cam_devices(&mut st, &mut sh), BTreeSet::new());
        // The job begins: one device per camera on the riser, in the Net's order.
        crate::venues::begin(&mut st, b, copy.inputs).unwrap();
        crack_server(&mut st, b);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_eq!(sh.user, "root");
        let net = crate::sitenet::net_of(&st, b).unwrap();
        let riser: Vec<citygen::SiteNode> = (net.nodes.iter())
            .filter(|n| n.kind == NodeKind::Camera && net.on_riser(n.node.level))
            .map(|n| n.node)
            .collect();
        let n = riser.len();
        assert!(n > 1, "cameras up the riser");
        let want: BTreeSet<String> = (0..n).map(|i| format!("cam{i}")).collect();
        assert_eq!(cam_devices(&mut st, &mut sh), want);
        // Each loops its own camera.
        let k = n - 1;
        let out = text(&go(&mut st, &mut sh, &format!("echo loop > /dev/cam{k}")));
        assert_eq!(out, "1 CAMERA LOOPED · 60 MIN");
        let now = st.net.now(&st);
        for (i, c) in riser.iter().enumerate() {
            let blind = st.net.blinded(citygen::NetNode::Site(*c), now);
            assert_eq!(blind, i == k, "cam{i}");
        }
        let cat = text(&go(&mut st, &mut sh, &format!("cat /dev/cam{k}")));
        assert!(cat.starts_with(&format!("cam{k}: looped (until ")), "{cat}");
        // The job ends: its cameras go from the host's list.
        crate::venues::end(&mut st, b).unwrap();
        let mut sh = new_shell(&mut st, HostId::Building(b));
        assert_eq!(cam_devices(&mut st, &mut sh), BTreeSet::new());
    }

    #[test]
    fn a_honeypot_logs_you_in_and_rings_the_alarm() {
        let (mut st, b, d) = state();
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(b, 1, seed, 0);
        st.backdoors.list[0].state = crate::backdoors::State::Honeypot;
        let mut sh = new_shell(&mut st, HostId::District(d));
        // the clue is in the log before you use it
        let mut peek = new_shell(&mut st, HostId::Building(b));
        let log = text(&go(&mut st, &mut peek, "cat /var/log/auth.log"));
        assert!(log.contains("audit  key flagged · monitoring"), "{log}");
        let n = hostname(&st, HostId::Building(b));
        let out = text(&go(&mut st, &mut sh, &format!("ssh {n}")));
        assert!(
            out.contains("AUDIT · THIS SESSION IS BEING RECORDED"),
            "{out}"
        );
        assert_eq!(sh.user, "root");
        let now = st.net.now(&st);
        assert!(st.net.alert(b, now));
        // from a street terminal no trace runs home
        assert!(st.backdoors.trace.is_none());
        // from the rig it does
        let mut st = state_home();
        let home = st.home.unwrap().home.building;
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(home, 1, seed, 0);
        st.backdoors.list[0].state = crate::backdoors::State::Honeypot;
        let mut sh = new_shell(&mut st, HostId::Rig);
        go(&mut st, &mut sh, "ssh 1");
        assert!(st.backdoors.trace.is_some());
        assert!(st.backdoors.trace.as_ref().unwrap().relays.is_empty());
    }
}
