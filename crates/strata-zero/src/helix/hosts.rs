//! Generating a host's disk from the city: residents, mail, notes, devices and lore.

use super::shell::HostId;
use super::vfs::*;
use super::{Host, User};
use crate::access::{Tier, building_label, tier_on};
use crate::app::AppState;
use crate::credentials::{self, Code, Source};
use crate::ui::device::{Device, Look};
use citygen::rng::{door_code, hash64};
use citygen::{BuildingId, DistrictId, NodeKind};
use sim::{HouseholdId, PersonId};
use std::collections::BTreeSet;

/// Lower case, non-alphanumerics to `-`, runs collapsed, ends trimmed.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// The rig user: your handle slugged.
fn rig_user(st: &AppState) -> String {
    let s = slug(&st.character.handle);
    if s.is_empty() {
        "runner".to_string()
    } else {
        s
    }
}

pub fn hostname(st: &AppState, id: HostId) -> String {
    match id {
        HostId::Rig => "zenbo".to_string(),
        HostId::Building(b) => slug(&building_label(st, b)),
        HostId::District(d) => st
            .city
            .as_ref()
            .and_then(|c| c.district_opt(d))
            .map(|d| slug(&d.name))
            .unwrap_or_else(|| "terminal".to_string()),
    }
}

/// A resident household of a building host: who, where and under which user name.
struct Resident {
    household: HouseholdId,
    person: PersonId,
    flat: (i32, u16),
    user: String,
    name: String,
}

/// The household heads of a building, in household order, with unique user names.
fn residents(st: &AppState, b: BuildingId) -> Vec<Resident> {
    let (Some(homes), Some(world)) = (st.interiors.homes_of(b), st.world.as_ref()) else {
        return Vec::new();
    };
    let mut taken: BTreeSet<String> = ["root", "guest", "user"].map(String::from).into();
    let mut out = Vec::new();
    for (household, flat) in homes.households() {
        let Some(person) = homes.head_of(household) else {
            continue;
        };
        let name = world.people()[person.0 as usize].name.clone();
        let mut user = slug(&name);
        if user.is_empty() {
            user = format!("flat-{}", flat.1);
        }
        if taken.contains(&user) {
            user = format!("{user}-{}", flat.1);
        }
        taken.insert(user.clone());
        out.push(Resident {
            household,
            person,
            flat,
            user,
            name,
        });
    }
    out
}

/// The login user by tier: root, a resident's user, `user` or `guest`; on the rig, you.
pub fn user_for(st: &AppState, id: HostId) -> String {
    match id {
        HostId::Rig => rig_user(st),
        HostId::District(_) => GUEST.to_string(),
        HostId::Building(b) => match tier_on(st, b) {
            Tier::Admin => ROOT.to_string(),
            Tier::Guest => GUEST.to_string(),
            Tier::User => {
                let card = st
                    .character
                    .inventory
                    .iter()
                    .filter_map(|s| credentials::parse_keycard(&s.item))
                    .find(|(cb, _)| *cb == b)
                    .map(|(_, h)| h);
                let coded = st
                    .journal
                    .codes()
                    .iter()
                    .filter(|c| c.building == b)
                    .find_map(|c| c.household);
                card.or(coded)
                    .and_then(|h| residents(st, b).into_iter().find(|r| r.household == h))
                    .map(|r| r.user)
                    .unwrap_or_else(|| "user".to_string())
            }
        },
    }
}

/// A host's disk, generated on first touch. `Building` hosts need `credentials::prepare`
/// to have run for their residents to exist, which this does first.
pub fn ensure_host(st: &mut AppState, id: HostId) -> &mut Host {
    if let HostId::Building(b) = id {
        credentials::prepare(st, b);
    }
    if !st.helix.hosts.contains_key(&id) {
        let host = generate(st, id);
        st.helix.hosts.insert(id, host);
    }
    let adds = if id == HostId::Rig {
        rig_mails(st)
    } else {
        Vec::new()
    };
    let host = st.helix.hosts.entry(id).or_default();
    for (path, index, owner) in adds {
        if host.fs.resolve("/", "/", &path).is_err() {
            host.fs
                .put_dev(&path, Dev::Mail(index), &owner, Mode::RW_NONE);
        }
    }
    host
}

/// (path, mail index, owner) for every mail in the rig's inbox.
fn rig_mails(st: &AppState) -> Vec<(String, usize, String)> {
    let me = rig_user(st);
    st.journal
        .mails
        .iter()
        .enumerate()
        .map(|(i, m)| {
            (
                format!("/home/{me}/mail/{:03}-{}.txt", i + 1, slug(&m.subject)),
                i,
                me.clone(),
            )
        })
        .collect()
}

pub fn device_of(id: HostId) -> Device {
    match id {
        HostId::Rig => Device::Rig,
        HostId::Building(b) => Device::Keypad(b),
        HostId::District(d) => Device::Terminal(d),
    }
}

/// The host the state implies, with no side effects.
pub fn generate(st: &AppState, id: HostId) -> Host {
    let look = Look::pick(st, device_of(id));
    let theme = look.name.clone();
    let name = hostname(st, id);
    let mut host = Host {
        name: name.clone(),
        fs: Vfs::new(),
        users: Vec::new(),
        theme,
    };
    let fs = &mut host.fs;
    fs.mkdir_as("/bin", ROOT, Mode::RW_R);
    fs.mkdir_as("/etc", ROOT, Mode::RW_R);
    fs.mkdir_as("/root", ROOT, Mode::RW_NONE);
    fs.mkdir_as("/tmp", ROOT, Mode::RW_RW);
    fs.put("/etc/hostname", &format!("{name}\n"), ROOT, Mode::RW_R);
    fs.put_dev("/etc/passwd", Dev::Passwd, ROOT, Mode::RW_R);
    host.users.push(User {
        name: ROOT.into(),
        home: "/root".into(),
        display: ROOT.into(),
        person: None,
    });
    let mut first_resident = None;
    match id {
        HostId::Rig => rig_disk(st, &look, &mut host),
        HostId::Building(b) => {
            first_resident = building_disk(st, b, &look, &mut host);
        }
        HostId::District(d) => district_disk(st, d, &look, &mut host),
    }
    lore(st, id, first_resident.as_deref(), &mut host);
    host
}

fn motd(st: &AppState, look: &Look, place: &str) -> String {
    let _ = st;
    format!(
        "{} · {}\n{place}\nAuthorised users only. Sessions are logged.\n",
        look.terminal_badge(),
        look.os()
    )
}

fn rig_disk(st: &AppState, look: &Look, host: &mut Host) {
    let me = rig_user(st);
    let home = format!("/home/{me}");
    host.users.push(User {
        name: me.clone(),
        home: home.clone(),
        display: st.character.handle.clone(),
        person: None,
    });
    let fs = &mut host.fs;
    // the rig's files are yours, /etc and /bin too
    fs.mkdir_as("/bin", &me, Mode::RW_R);
    fs.mkdir_as("/etc", &me, Mode::RW_R);
    fs.mkdir_as("/tmp", &me, Mode::RW_RW);
    fs.mkdir_as(&home, &me, Mode::RW_NONE);
    fs.mkdir_as(&format!("{home}/mail"), &me, Mode::RW_NONE);
    fs.put(
        "/etc/motd",
        &motd(st, look, "ZENBO rig · your flat"),
        &me,
        Mode::RW_R,
    );
    fs.put("/etc/hostname", "zenbo\n", &me, Mode::RW_R);
    fs.put_dev("/etc/passwd", Dev::Passwd, &me, Mode::RW_R);
    fs.put_dev(
        &format!("{home}/journal.txt"),
        Dev::Journal,
        &me,
        Mode::RW_NONE,
    );
    fs.put(&format!("{home}/notes.txt"), "", &me, Mode::RW_NONE);
    for (path, index, owner) in rig_mails(st) {
        fs.put_dev(&path, Dev::Mail(index), &owner, Mode::RW_NONE);
    }
    fs.mkdir_as("/net", &me, Mode::RW_R);
    if let Some(rig) = st.home {
        let b = rig.home.building;
        fs.put_dev(
            &format!("/net/{}", slug(&building_label(st, b))),
            Dev::NetHost(b),
            USERS,
            Mode::RW_R,
        );
    }
}

fn district_disk(st: &AppState, d: DistrictId, look: &Look, host: &mut Host) {
    host.users.push(User {
        name: GUEST.into(),
        home: "/tmp".into(),
        display: GUEST.into(),
        person: None,
    });
    let place = st
        .city
        .as_ref()
        .and_then(|c| c.district_opt(d))
        .map(|d| d.name.to_uppercase())
        .unwrap_or_default();
    let fs = &mut host.fs;
    fs.put("/etc/motd", &motd(st, look, &place), ROOT, Mode::RW_R);
    fs.mkdir_as("/net", ROOT, Mode::RW_R);
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let servers: BTreeSet<BuildingId> = city
        .net
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Server)
        .filter_map(|n| n.building)
        .collect();
    let mut used = BTreeSet::new();
    for b in &city.buildings {
        if b.district != d || !b.kind.is_closed() || !servers.contains(&b.id) {
            continue;
        }
        let mut entry = slug(&building_label(st, b.id));
        if !used.insert(entry.clone()) {
            entry = format!("{entry}-{}", b.id.index());
        }
        fs.put_dev(
            &format!("/net/{entry}"),
            Dev::NetHost(b.id),
            USERS,
            Mode::RW_R,
        );
    }
}

/// The building's disk; returns the first resident's user for `{resident}` paths.
fn building_disk(st: &AppState, b: BuildingId, look: &Look, host: &mut Host) -> Option<String> {
    let label = building_label(st, b);
    for (n, home) in [(GUEST, "/tmp"), ("user", "/tmp")] {
        host.users.push(User {
            name: n.into(),
            home: home.into(),
            display: n.into(),
            person: None,
        });
    }
    host.fs
        .put("/etc/motd", &motd(st, look, &label), ROOT, Mode::RW_R);
    let fs = &mut host.fs;
    fs.mkdir_as("/var/log", ROOT, Mode::RW_R);
    fs.put_dev("/var/log/auth.log", Dev::AuthLog, ROOT, Mode::RW_R);
    fs.put_dev("/var/log/net.log", Dev::NetLog, ROOT, Mode::RW_R);
    fs.mkdir_as("/var/plan", ROOT, Mode::R_R);
    fs.put_dev("/var/plan/ground", Dev::Plan, ROOT, Mode::R_R);
    fs.mkdir_as("/dev", ROOT, Mode::RW_R);
    fs.put_dev("/dev/door", Dev::Door, USERS, Mode::RW_R);
    fs.put_dev("/dev/lift", Dev::Lift, USERS, Mode::RW_R);
    let cams = st.city.as_ref().map_or(0, |c| {
        c.net
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Camera && n.building == Some(b))
            .count()
    });
    for i in 0..cams {
        fs.put_dev(&format!("/dev/cam{i}"), Dev::Cam(i), USERS, Mode::RW_R);
    }
    fs.put_dev("/dev/alarm", Dev::Alarm, ROOT, Mode::RW_R);

    let seed = st.city.as_ref().map_or(0, |c| c.seed);
    let rs = residents(st, b);
    for (i, r) in rs.iter().enumerate() {
        host.users.push(User {
            name: r.user.clone(),
            home: format!("/home/{}", r.user),
            display: r.name.clone(),
            person: Some(r.person),
        });
        let home = format!("/home/{}", r.user);
        let fs = &mut host.fs;
        fs.mkdir_as(&home, &r.user, Mode::RW_NONE);
        fs.mkdir_as(&format!("{home}/mail"), &r.user, Mode::RW_NONE);
        let neighbour = if rs.len() > 1 {
            let k = hash64(seed ^ 0x4E1B, r.household.0 as u64) as usize % (rs.len() - 1);
            let j = if k >= i { k + 1 } else { k };
            Some(&rs[j])
        } else {
            None
        };
        let nb_name = neighbour.map_or("the super", |n| n.name.as_str());
        let count = 1 + (hash64(seed ^ 0x3A11, r.household.0 as u64) % 2) as usize;
        for n in 0..count {
            let pick = hash64(
                seed ^ (0x3A12 + n as u64),
                r.household.0 as u64 ^ b.index() as u64,
            );
            let (from, subject, body) = mail_for(pick, &r.name, nb_name, &label);
            fs.put(
                &format!("{home}/mail/{:03}.txt", n + 1),
                &format!(
                    "From: {from}\nTo: {}\nSubject: {subject}\n\n{body}\n",
                    r.name
                ),
                &r.user,
                Mode::RW_NONE,
            );
        }
        if hash64(
            seed ^ 0x2073,
            r.household.0 as u64 ^ ((b.index() as u64) << 20),
        )
        .is_multiple_of(3)
        {
            let text = notes_for(seed, b, r, neighbour);
            fs.put(&format!("{home}/notes.txt"), &text, &r.user, Mode::RW_NONE);
        }
    }
    // /etc/passwd is rendered from `users`; nothing more to store
    rs.first().map(|r| r.user.clone())
}

/// A generated mail: (from, subject, body). One in ten is about the lobby's lost card.
fn mail_for(pick: u64, me: &str, neighbour: &str, building: &str) -> (String, String, String) {
    let first = me.split_whitespace().next().unwrap_or(me);
    if pick.is_multiple_of(10) {
        return (
            "Front desk".into(),
            "A card at the desk".into(),
            format!(
                "{first}, somebody left a keycard on the lobby desk again.\nIf it is yours, come and sign for it. If it is not, do not."
            ),
        );
    }
    match (pick / 10) % 4 {
        0 => (
            format!("{building} management"),
            "Rent".into(),
            format!("{first}, rent is due on the first.\nThe office will not chase you twice."),
        ),
        1 => (
            neighbour.into(),
            "Saturday".into(),
            format!(
                "{first}, people are coming over Saturday. Bring noodles.\nSorry in advance about the noise.\n{neighbour}"
            ),
        ),
        2 => (
            format!("{building} management"),
            "The lift".into(),
            format!(
                "{first}, the lift is out again. Engineers promise Thursday.\nPlease use the stairs and stop propping the door."
            ),
        ),
        _ => (
            neighbour.into(),
            "Your music".into(),
            format!("{first}, it comes through the wall. Not at night, ok?\n{neighbour}"),
        ),
    }
}

const NOTE_LINES: &[&str] = &[
    "buy noodles. buy the good ones this time.",
    "call about the lift. again.",
    "do not forget the rent. the first.",
    "borrowed a shard from a friend, return it",
    "new neighbour is loud",
    "stop leaving the window open",
];

/// A few lines in the resident's voice; maybe their door code, maybe a neighbour's.
fn notes_for(seed: u64, b: BuildingId, r: &Resident, neighbour: Option<&Resident>) -> String {
    let h = hash64(
        seed ^ 0x707E,
        r.household.0 as u64 ^ ((b.index() as u64) << 20),
    );
    let mut lines: Vec<String> = Vec::new();
    for k in 0..2 {
        lines.push(NOTE_LINES[(hash64(h, k) % NOTE_LINES.len() as u64) as usize].to_string());
    }
    if (h >> 8).is_multiple_of(3) {
        let code = door_code(seed, b, r.flat.0, r.flat.1);
        lines.push(format!("door is {code:04}, don't tell the super"));
        if (h >> 16).is_multiple_of(3)
            && let Some(n) = neighbour
        {
            let code = door_code(seed, b, n.flat.0, n.flat.1);
            lines.push(format!("{}'s is {code:04}", n.name));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Lore from the content file whose theme and kind match the host.
fn lore(st: &AppState, id: HostId, resident: Option<&str>, host: &mut Host) {
    let kind = match id {
        HostId::Building(b) => st
            .city
            .as_ref()
            .and_then(|c| c.building_opt(b))
            .map(|x| x.kind.label()),
        _ => None,
    };
    let owner = if id == HostId::Rig {
        rig_user(st)
    } else {
        ROOT.to_string()
    };
    for l in &st.content.lore {
        let theme_ok = l.theme.as_deref().is_none_or(|t| t == host.theme);
        let kind_ok = l
            .kind
            .as_deref()
            .is_none_or(|k| kind.is_some_and(|have| have.eq_ignore_ascii_case(k)));
        if !theme_ok || !kind_ok {
            continue;
        }
        let (path, who) = if l.path.contains("{resident}") {
            let Some(r) = resident else {
                continue;
            };
            (l.path.replace("{resident}", r), r.to_string())
        } else {
            (l.path.clone(), owner.clone())
        };
        host.fs.put(&path, &l.text, &who, Mode::RW_R);
    }
}

/// Reading a file on a building host: every four-digit run that is the door code of one
/// of its flats is learned, once, as read in the file owner's notes.
pub fn note_codes(st: &mut AppState, host: HostId, path: &str, text: &str) {
    let HostId::Building(b) = host else {
        return;
    };
    let Some(seed) = st.city.as_ref().map(|c| c.seed) else {
        return;
    };
    let Some(homes) = st.interiors.homes_of(b).cloned() else {
        return;
    };
    let mut numbers: Vec<u16> = Vec::new();
    // Only lines that talk about a door or a code count: a year in a mail is not a code.
    let relevant: String = text
        .lines()
        .filter(|l| {
            let l = l.to_lowercase();
            l.contains("door") || l.contains("code")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let bytes: Vec<char> = relevant.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i - start == 4
                && let Ok(n) = bytes[start..i].iter().collect::<String>().parse()
            {
                numbers.push(n);
            }
        } else {
            i += 1;
        }
    }
    let owner = path
        .strip_prefix("/home/")
        .and_then(|p| p.split('/').next())
        .and_then(|u| residents(st, b).into_iter().find(|r| r.user == u))
        .map(|r| r.name);
    let now = st.net.now(st);
    for n in numbers {
        for (level, number) in homes.flats() {
            if door_code(seed, b, level, number) != n {
                continue;
            }
            let learned = st.journal.learn(Code {
                building: b,
                level,
                number,
                household: homes.household_of(level, number),
                code: n,
                source: Source::Shell,
                at: now,
            });
            if learned {
                let whose = owner.clone().unwrap_or_else(|| "someone".to_string());
                st.say(format!("a code in `{whose}`'s notes"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::Source;
    use citygen::{BuildingId, BuildingKind, GenParams, Generator, SizePreset};

    fn state() -> AppState {
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
        st
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
        st
    }

    /// Residential buildings with residents, in id order.
    fn residential(st: &AppState) -> Vec<BuildingId> {
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        city.buildings
            .iter()
            .filter(|b| {
                matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
                    && world.people().iter().any(|p| p.home == Some(b.id))
            })
            .map(|b| b.id)
            .collect()
    }

    fn exists(h: &Host, path: &str) -> bool {
        h.fs.resolve("/", "/", path).is_ok()
    }

    fn kind<'a>(h: &'a Host, path: &str) -> &'a Kind {
        &h.fs.node(h.fs.resolve("/", "/", path).unwrap()).kind
    }

    fn text(h: &Host, path: &str) -> String {
        match kind(h, path) {
            Kind::File(t) => t.clone(),
            other => panic!("{path} is {other:?}"),
        }
    }

    fn names(h: &Host, dir: &str) -> Vec<String> {
        match kind(h, dir) {
            Kind::Dir(c) => c.keys().cloned().collect(),
            other => panic!("{dir} is {other:?}"),
        }
    }

    #[test]
    fn a_building_host_has_its_residents_and_their_homes() {
        let mut st = state();
        let bs = residential(&st);
        let b = bs[0];
        let host = ensure_host(&mut st, HostId::Building(b)).clone();
        assert!(host.users.iter().any(|u| u.name == "root"));
        assert!(host.users.iter().any(|u| u.name == "guest"));
        assert!(host.users.iter().any(|u| u.name == "user"));
        let residents: Vec<&User> = host.users.iter().filter(|u| u.person.is_some()).collect();
        assert!(!residents.is_empty());
        assert!(matches!(kind(&host, "/etc/passwd"), Kind::Dev(Dev::Passwd)));
        assert_eq!(text(&host, "/etc/hostname").trim(), host.name);
        for r in &residents {
            let id = host.fs.resolve("/", "/", &r.home).unwrap();
            let n = host.fs.node(id);
            assert_eq!(n.owner, r.name);
            assert_eq!(n.mode, Mode::RW_NONE);
            let mails = names(&host, &format!("{}/mail", r.home));
            assert!((1..=2).contains(&mails.len()), "{mails:?}");
            assert_eq!(mails[0], "001.txt");
            assert!(text(&host, &format!("{}/mail/001.txt", r.home)).contains(&r.display));
        }
        assert!(matches!(kind(&host, "/dev/door"), Kind::Dev(Dev::Door)));
        assert_eq!(
            host.fs
                .node(host.fs.resolve("/", "/", "/dev/alarm").unwrap())
                .owner,
            ROOT
        );
        assert_eq!(
            host.fs
                .node(host.fs.resolve("/", "/", "/tmp").unwrap())
                .mode,
            Mode::RW_RW
        );
        assert!(exists(&host, "/var/log/auth.log") && exists(&host, "/var/plan/ground"));
        // at least one notes.txt among the first few residential hosts
        let mut notes = 0;
        for b in bs.iter().take(6) {
            let h = ensure_host(&mut st, HostId::Building(*b)).clone();
            notes += h
                .users
                .iter()
                .filter(|u| u.person.is_some() && exists(&h, &format!("{}/notes.txt", u.home)))
                .count();
        }
        assert!(notes > 0);
        // same seed, identical disk
        let again = generate(&st, HostId::Building(b));
        assert_eq!(again.fs, host.fs);
        let st2 = {
            let mut s = state();
            ensure_host(&mut s, HostId::Building(b));
            s
        };
        assert_eq!(st2.helix.hosts[&HostId::Building(b)].fs, host.fs);
    }

    #[test]
    fn notes_hold_a_code_and_reading_learns_it_once() {
        let mut st = state();
        let mut found = None;
        'all: for b in residential(&st) {
            let h = ensure_host(&mut st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let path = format!("{}/notes.txt", u.home);
                if exists(&h, &path) && text(&h, &path).contains("door is") {
                    found = Some((b, path.clone(), text(&h, &path)));
                    break 'all;
                }
            }
        }
        let (b, path, body) = found.expect("a notes file with a code");
        assert!(st.journal.codes().is_empty());
        note_codes(&mut st, HostId::Building(b), &path, &body);
        let n = st.journal.codes().len();
        assert!(n >= 1);
        note_codes(&mut st, HostId::Building(b), &path, &body);
        assert_eq!(st.journal.codes().len(), n);
        assert!(st.journal.codes().iter().all(|c| c.source == Source::Shell));
        assert!(st.journal.codes().iter().all(|c| c.household.is_some()));
        let seed = st.city.as_ref().unwrap().seed;
        let c = &st.journal.codes()[0];
        assert_eq!(c.code, citygen::rng::door_code(seed, b, c.level, c.number));
        let said = st.log.iter().filter(|e| e.text.contains("notes")).count();
        assert_eq!(said, n);
    }

    #[test]
    fn only_door_and_code_lines_teach_codes() {
        let mut st = state();
        let mut found = None;
        'all: for b in residential(&st) {
            let h = ensure_host(&mut st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let path = format!("{}/notes.txt", u.home);
                if exists(&h, &path) && text(&h, &path).contains("door is") {
                    found = Some((b, path.clone(), text(&h, &path)));
                    break 'all;
                }
            }
        }
        let (b, path, body) = found.expect("a notes file with a code");
        let code = body
            .split(|c: char| !c.is_ascii_digit())
            .find(|w| w.len() == 4)
            .expect("a four-digit code")
            .to_string();
        let year = format!("Back in 2087 we met, ref {code}.");
        note_codes(&mut st, HostId::Building(b), &path, &year);
        assert!(st.journal.codes().is_empty(), "a year line taught a code");
        note_codes(
            &mut st,
            HostId::Building(b),
            &path,
            &format!("the door code is {code}"),
        );
        assert!(!st.journal.codes().is_empty());
    }

    #[test]
    fn the_rig_has_your_mail_and_journal() {
        let mut st = state_home();
        let home = st.home.unwrap().home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        let me = user_for(&st, HostId::Rig);
        assert_eq!(me, slug(&st.character.handle));
        let host = ensure_host(&mut st, HostId::Rig).clone();
        let mail = format!("/home/{me}/mail/001-a-favour.txt");
        assert_eq!(kind(&host, &mail), &Kind::Dev(Dev::Mail(0)));
        assert_eq!(
            kind(&host, &format!("/home/{me}/journal.txt")),
            &Kind::Dev(Dev::Journal)
        );
        let notes = host
            .fs
            .resolve("/", "/", &format!("/home/{me}/notes.txt"))
            .unwrap();
        assert!(host.fs.can_write(notes, &me));
        assert!(!host.fs.can_read(notes, GUEST));
        assert!(
            host.fs
                .can_write(host.fs.resolve("/", "/", "/etc/motd").unwrap(), &me)
        );
        assert!(text(&host, "/etc/motd").contains("ZENBO"));
        assert_eq!(names(&host, "/net").len(), 1);
        // a later mail gets a file; the first stays
        st.journal.mails.push(crate::credentials::Mail {
            from: "Kit".into(),
            subject: "Re: Drinks".into(),
            body: vec![],
            read: false,
            at: 9,
        });
        let host = ensure_host(&mut st, HostId::Rig).clone();
        assert_eq!(
            kind(&host, &format!("/home/{me}/mail/002-re-drinks.txt")),
            &Kind::Dev(Dev::Mail(1))
        );
        assert!(exists(&host, &mail));
    }

    #[test]
    fn lore_lands_where_it_matches() {
        let mut st = state();
        let city = st.city.as_ref().unwrap();
        let by_theme = |want: &str| {
            let ids: Vec<BuildingId> = city.buildings.iter().map(|b| b.id).collect();
            ids.into_iter().find(|b| {
                crate::ui::device::Look::pick(&st, crate::ui::device::Device::Keypad(*b)).name
                    == want
            })
        };
        let corp = by_theme("megacorp").expect("a megacorp building");
        let street = by_theme("street").expect("a street building");
        let corp = ensure_host(&mut st, HostId::Building(corp)).clone();
        let street = ensure_host(&mut st, HostId::Building(street)).clone();
        assert_eq!(corp.theme, "megacorp");
        assert!(exists(&corp, "/etc/security-policy.txt"));
        assert!(!exists(&corp, "/tmp/zine.txt"));
        assert!(exists(&street, "/tmp/zine.txt"));
        assert!(!exists(&street, "/etc/security-policy.txt"));
    }

    #[test]
    fn user_for_follows_the_tier() {
        let mut st = state();
        let b = residential(&st)[0];
        crate::credentials::prepare(&mut st, b);
        let id = HostId::Building(b);
        assert_eq!(user_for(&st, id), "guest");
        let host = ensure_host(&mut st, id).clone();
        let r = host
            .users
            .iter()
            .find(|u| u.person.is_some())
            .unwrap()
            .clone();
        // a code with no household
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let (h, (level, number)) = homes.households().next().unwrap();
        st.journal.learn(crate::credentials::Code {
            building: b,
            level,
            number,
            household: None,
            code: 1,
            source: Source::Keypad,
            at: 0,
        });
        assert_eq!(user_for(&st, id), "user");
        // a card
        st.character.give(crate::credentials::keycard_id(b, h), 1);
        assert_eq!(
            user_for(&st, id),
            slug(&st.world.as_ref().unwrap().people()[homes.head_of(h).unwrap().0 as usize].name)
        );
        assert!(host.users.iter().any(|u| u.name == user_for(&st, id)));
        let _ = r;
        // the backdoor
        st.net.backdoors.insert(b);
        assert_eq!(user_for(&st, id), "root");
        // a street terminal
        let d = st.city.as_ref().unwrap().districts[0].id;
        assert_eq!(user_for(&st, HostId::District(d)), "guest");
    }

    #[test]
    fn slug_and_hostname() {
        assert_eq!(slug("Hollis Court"), "hollis-court");
        assert_eq!(slug("APARTMENT 78"), "apartment-78");
        assert_eq!(slug("  --Low  Town!! "), "low-town");
        let st = state_home();
        assert_eq!(hostname(&st, HostId::Rig), "zenbo");
        let b = st.home.unwrap().home.building;
        let label = crate::access::building_label(&st, b);
        assert_eq!(hostname(&st, HostId::Building(b)), slug(&label));
        let d = st.city.as_ref().unwrap().districts[0].id;
        assert_eq!(
            hostname(&st, HostId::District(d)),
            slug(&st.city.as_ref().unwrap().district(d).name)
        );
    }
}
