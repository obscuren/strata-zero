//! Generating a host's disk from the city: residents, mail, notes, devices and lore.

use super::shell::HostId;
use super::vfs::*;
use super::{Host, Planted, User};
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
pub(super) fn rig_user(st: &AppState) -> String {
    let s = slug(&st.character.handle);
    if s.is_empty() {
        "runner".to_string()
    } else {
        s
    }
}

/// The name a building's server goes by on the Net.
pub fn building_hostname(st: &AppState, b: citygen::BuildingId) -> String {
    hostname(st, HostId::Building(b))
}

pub fn hostname(st: &AppState, id: HostId) -> String {
    match id {
        HostId::Rig => "zenbo".to_string(),
        HostId::Building(b) => st
            .city
            .as_ref()
            .and_then(|c| c.building_opt(b))
            .and_then(|x| {
                net_names(st, x.district)
                    .into_iter()
                    .find(|(y, _)| *y == b)
                    .map(|(_, n)| n)
            })
            .unwrap_or_else(|| slug(&building_label(st, b))),
        HostId::District(d) => st
            .city
            .as_ref()
            .and_then(|c| c.district_opt(d))
            .map(|d| slug(&d.name))
            .unwrap_or_else(|| "terminal".to_string()),
    }
}

/// The names a district's constructs go by on its net, in building order: the slug of
/// the building's name, and for a second building of the same name `-<its index>` after.
pub fn net_names(st: &AppState, district: DistrictId) -> Vec<(BuildingId, String)> {
    let mut used = BTreeSet::new();
    crate::access::scannable(st, district)
        .into_iter()
        .map(|b| {
            let mut name = slug(&building_label(st, b));
            if !used.insert(name.clone()) {
                name = format!("{name}-{}", b.index());
            }
            (b, name)
        })
        .collect()
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

fn trapped(st: &AppState, b: BuildingId) -> Option<String> {
    st.backdoors
        .trap_login
        .as_ref()
        .filter(|(tb, _)| *tb == b)
        .map(|(_, u)| u.clone())
}

/// The login user by tier: root, a resident's user, `user` or `guest`; on the rig, you.
pub fn user_for(st: &AppState, id: HostId) -> String {
    if let HostId::Building(b) = id
        && let Some(user) = trapped(st, b)
    {
        return user;
    }
    match id {
        HostId::Rig => rig_user(st),
        HostId::District(_) => GUEST.to_string(),
        HostId::Building(b) => match tier_on(st, b) {
            Tier::Admin => ROOT.to_string(),
            Tier::Guest => GUEST.to_string(),
            Tier::User => {
                if let Some(crate::backdoors::Kind::Key { user }) =
                    st.backdoors.live(b).map(|d| d.kind.clone())
                {
                    return user;
                }
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
    if id == HostId::Rig {
        sync_rig_net(st);
    }
    let host = st.helix.hosts.entry(id).or_default();
    // only mails that arrived since the last look: one you deleted stays deleted
    for (path, index, owner) in adds.into_iter().skip(host.mails_placed) {
        if host.fs.resolve("/", "/", &path).is_err() {
            host.fs
                .put_dev(&path, Dev::Mail(index), &owner, Mode::RW_NONE);
        }
        host.mails_placed = index + 1;
    }
    host
}

/// Keeps the rig's `/net` in step with its lines: the home block and every backdoor
/// not burned, one entry each; a burned one's entry goes.
fn sync_rig_net(st: &mut AppState) {
    let want: BTreeSet<BuildingId> = st
        .home
        .map(|h| h.home.building)
        .into_iter()
        .chain(st.backdoors.standing().map(|d| d.building))
        .collect();
    let Some(placed) = st
        .helix
        .hosts
        .get(&HostId::Rig)
        .map(|h| h.nets_placed.clone())
    else {
        return;
    };
    // every name needed, worked out before the host is borrowed mutably
    let names: Vec<(BuildingId, String)> = want
        .union(&placed)
        .map(|b| (*b, hostname(st, HostId::Building(*b))))
        .collect();
    let Some(host) = st.helix.hosts.get_mut(&HostId::Rig) else {
        return;
    };
    for (b, name) in &names {
        let path = format!("/net/{name}");
        if want.contains(b) && !host.nets_placed.contains(b) {
            host.fs.put_dev(&path, Dev::NetHost(*b), USERS, Mode::RW_R);
            host.nets_placed.insert(*b);
        } else if !want.contains(b) && host.nets_placed.remove(b) {
            let _ = host.fs.remove(ROOT, "/", "/", &path, false);
        }
    }
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
        planted: Vec::new(),
        mails_placed: 0,
        nets_placed: BTreeSet::new(),
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
        host.fs
            .put_dev(&path, Dev::Mail(index), &owner, Mode::RW_NONE);
        host.mails_placed = index + 1;
    }
    let fs = &mut host.fs;
    fs.mkdir_as("/net", &me, Mode::RW_R);
    fs.mkdir_as("/var/log", &me, Mode::RW_R);
    fs.put_dev("/var/log/net.log", Dev::NetLog, &me, Mode::RW_R);
    if let Some(rig) = st.home {
        let b = rig.home.building;
        fs.put_dev(
            &format!("/net/{}", hostname(st, HostId::Building(b))),
            Dev::NetHost(b),
            USERS,
            Mode::RW_R,
        );
        host.nets_placed.insert(b);
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
    for (b, entry) in net_names(st, d) {
        fs.put_dev(&format!("/net/{entry}"), Dev::NetHost(b), USERS, Mode::RW_R);
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
            let path = format!("{home}/notes.txt");
            let (text, codes) = notes_for(seed, b, r, neighbour);
            fs.put(&path, &text, &r.user, Mode::RW_NONE);
            host.planted
                .extend(codes.into_iter().map(|(flat, code)| Planted {
                    flat,
                    code,
                    path: path.clone(),
                    by: r.name.clone(),
                }));
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

/// A flat, as (level, number), and its door code.
type FlatCode = ((i32, u16), u16);

/// A few lines in the resident's voice; maybe their door code, maybe a neighbour's.
/// The text, and the (flat, code) pairs it gives away.
fn notes_for(
    seed: u64,
    b: BuildingId,
    r: &Resident,
    neighbour: Option<&Resident>,
) -> (String, Vec<FlatCode>) {
    let h = hash64(
        seed ^ 0x707E,
        r.household.0 as u64 ^ ((b.index() as u64) << 20),
    );
    let mut lines: Vec<String> = Vec::new();
    let mut codes = Vec::new();
    for k in 0..2 {
        lines.push(NOTE_LINES[(hash64(h, k) % NOTE_LINES.len() as u64) as usize].to_string());
    }
    if (h >> 8).is_multiple_of(3) {
        let code = door_code(seed, b, r.flat.0, r.flat.1);
        lines.push(format!("door is {code:04}, don't tell the super"));
        codes.push((r.flat, code));
        if (h >> 16).is_multiple_of(3)
            && let Some(n) = neighbour
        {
            let code = door_code(seed, b, n.flat.0, n.flat.1);
            lines.push(format!("{}'s door is {code:04}", n.name));
            codes.push((n.flat, code));
        }
    }
    lines.push(String::new());
    (lines.join("\n"), codes)
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

/// Reading a file on a building host: every four-digit run on a line about a door or a
/// code that the generator planted in that very file is learned, once, as read in its
/// writer's notes. A file you wrote yourself, full of guesses, teaches nothing.
pub fn note_codes(st: &mut AppState, host: HostId, path: &str, text: &str) {
    let HostId::Building(b) = host else {
        return;
    };
    let Some(homes) = st.interiors.homes_of(b).cloned() else {
        return;
    };
    let planted: Vec<Planted> = st
        .helix
        .hosts
        .get(&host)
        .map(|h| {
            h.planted
                .iter()
                .filter(|p| p.path == path)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if planted.is_empty() {
        return;
    }
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
    let now = st.net.now(st);
    for n in numbers {
        for p in planted.iter().filter(|p| p.code == n) {
            let (level, number) = p.flat;
            let learned = st.journal.learn(Code {
                building: b,
                level,
                number,
                household: homes.household_of(level, number),
                code: n,
                source: Source::Shell,
                noted_by: Some(p.by.clone()),
                at: now,
            });
            if learned {
                st.say(format!("a code in `{}`'s notes", p.by));
            }
        }
    }
}

/// Test only: gives two scannable buildings of one district the same name;
/// (district, the first, the second) in building order.
#[cfg(test)]
pub(crate) fn twins(st: &mut AppState, name: &str) -> (DistrictId, BuildingId, BuildingId) {
    let city = st.city.as_ref().expect("a city");
    let (d, a, b) = city
        .districts
        .iter()
        .find_map(|d| match crate::access::scannable(st, d.id)[..] {
            [a, b, ..] => Some((d.id, a, b)),
            _ => None,
        })
        .expect("a district with two constructs");
    let city = st.city.as_mut().unwrap();
    for x in [a, b] {
        assert_eq!(city.buildings[x.index()].id, x);
        city.buildings[x.index()].name = Some(name.to_string());
    }
    (d, a, b)
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

    /// A four-digit run in `line`.
    fn four_digits(line: &str) -> Option<u16> {
        line.split(|c: char| !c.is_ascii_digit())
            .find(|w| w.len() == 4)
            .and_then(|w| w.parse().ok())
    }

    /// The first generated notes file that names a neighbour's code:
    /// (building, path, text, the notes' owner, the neighbour line).
    fn neighbour_notes(st: &mut AppState) -> Option<(BuildingId, String, String, String, String)> {
        for b in residential(st) {
            let h = ensure_host(st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let path = format!("{}/notes.txt", u.home);
                if !exists(&h, &path) {
                    continue;
                }
                let body = text(&h, &path);
                let line = body
                    .lines()
                    .find(|l| l.contains("'s ") && four_digits(l).is_some());
                if let Some(line) = line {
                    return Some((b, path, body.clone(), u.display.clone(), line.to_string()));
                }
            }
        }
        None
    }

    #[test]
    fn a_neighbours_code_in_notes_is_learned_and_credited_to_the_notes() {
        let mut st = state_home();
        let (b, path, body, owner, line) =
            neighbour_notes(&mut st).expect("a notes file with a neighbour's code");
        let code = four_digits(&line).unwrap();
        note_codes(&mut st, HostId::Building(b), &path, &body);
        let c = st
            .journal
            .codes()
            .iter()
            .find(|c| c.code == code)
            .unwrap_or_else(|| panic!("`{line}` taught nothing: {:?}", st.journal.codes()))
            .clone();
        assert_eq!(c.source, Source::Shell);
        let seed = st.city.as_ref().unwrap().seed;
        assert_eq!(c.code, citygen::rng::door_code(seed, b, c.level, c.number));
        // the journal says whose notes it was read in, not whose flat it opens
        let entry = credentials::entries(&st)
            .into_iter()
            .flat_map(|(_, _, es)| es)
            .find(|e| matches!(&e.kind, credentials::EntryKind::Code(k) if *k == c))
            .expect("a journal line");
        assert!(
            entry.text.contains(&format!("read in `{owner}`'s notes")),
            "{}",
            entry.text
        );
    }

    #[test]
    fn a_file_of_guesses_teaches_nothing() {
        let mut st = state();
        let mut found = None;
        'all: for b in residential(&st) {
            let h = ensure_host(&mut st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let path = format!("{}/notes.txt", u.home);
                if exists(&h, &path) && text(&h, &path).contains("door is") {
                    found = Some((b, path.clone(), text(&h, &path), u.home.clone()));
                    break 'all;
                }
            }
        }
        let (b, path, body, home) = found.expect("a notes file with a code");
        let seed = st.city.as_ref().unwrap().seed;
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let every: Vec<String> = homes
            .flats()
            .map(|(l, n)| format!("door code {:04}", citygen::rng::door_code(seed, b, l, n)))
            .collect();
        let guesses = every.join("\n");
        for at in ["/tmp/g", &format!("{home}/guess.txt")] {
            note_codes(&mut st, HostId::Building(b), at, &guesses);
            assert!(
                st.journal.codes().is_empty(),
                "{at} taught {:?}",
                st.journal.codes()
            );
        }
        // the planted notes still teach
        note_codes(&mut st, HostId::Building(b), &path, &body);
        assert!(!st.journal.codes().is_empty());
    }

    #[test]
    fn a_deleted_mail_stays_deleted() {
        let mut st = state_home();
        let home = st.home.unwrap().home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        crate::jobs::first_favour(&mut st);
        let me = user_for(&st, HostId::Rig);
        let mail = format!("/home/{me}/mail/001-a-favour.txt");
        ensure_host(&mut st, HostId::Rig)
            .fs
            .remove(&me, "/", "/", &mail, false)
            .unwrap();
        let host = ensure_host(&mut st, HostId::Rig).clone();
        assert!(!exists(&host, &mail), "the mail came back");
        assert!(names(&host, &format!("/home/{me}/mail")).is_empty());
        st.journal.mails.push(crate::credentials::Mail {
            from: "Kit".into(),
            subject: "Re: Drinks".into(),
            body: vec![],
            read: false,
            at: 9,
            job: None,
            story: None,
        });
        let host = ensure_host(&mut st, HostId::Rig).clone();
        assert_eq!(
            names(&host, &format!("/home/{me}/mail")),
            vec!["002-re-drinks.txt".to_string()]
        );
    }

    #[test]
    fn twin_names_on_one_subnet_get_distinct_hostnames() {
        let mut st = state();
        let (d, a, b) = twins(&mut st, "Twin Tower");
        let listed = net_names(&st, d);
        let of = |x: BuildingId| listed.iter().find(|(y, _)| *y == x).unwrap().1.clone();
        assert_eq!(of(a), "twin-tower");
        assert_eq!(of(b), format!("twin-tower-{}", b.index()));
        assert_eq!(hostname(&st, HostId::Building(a)), of(a));
        assert_eq!(hostname(&st, HostId::Building(b)), of(b));
        let host = ensure_host(&mut st, HostId::District(d)).clone();
        let net = names(&host, "/net");
        assert!(net.contains(&of(a)) && net.contains(&of(b)), "{net:?}");
    }

    #[test]
    fn the_rig_has_your_mail_and_journal() {
        let mut st = state_home();
        let home = st.home.unwrap().home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        crate::jobs::first_favour(&mut st);
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
            job: None,
            story: None,
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
            noted_by: None,
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
        st.backdoors.plant_net(b, 1, 0, 0);
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
