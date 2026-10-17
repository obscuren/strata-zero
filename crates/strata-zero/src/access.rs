//! The Access Terminal: a typed session on the machine itself. A district terminal
//! sees its subnet's buildings; the home rig sees your mail, your journal and your
//! block. Commands are plain functions over the app state that answer in text.

use crate::app::AppState;
use crate::credentials;
use citygen::construct::{BLACK_THRESHOLD, GREY_THRESHOLD, ICE_THRESHOLD};
use citygen::{BuildingId, City, DistrictId, NodeKind, Place, Pos, StratumKind, SubnetOwner};
use game::{HackTarget, Op, Target};
use sim::OccupantRole;

/// What the terminal lets you do on a building.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Guest,
    User,
    Admin,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::Guest => "GUEST",
            Tier::User => "USER",
            Tier::Admin => "ADMIN",
        }
    }

    fn needs(self) -> &'static str {
        match self {
            Tier::Guest => "nothing",
            Tier::User => "A RESIDENT'S CARD OR CODE",
            Tier::Admin => "A BACKDOOR ON THE SERVER",
        }
    }
}

/// Where a session sits and what it is connected to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    District {
        district: DistrictId,
        connected: Option<BuildingId>,
    },
    Rig {
        building: BuildingId,
    },
}

impl Session {
    /// The building commands act on, if any.
    fn building(&self) -> Option<BuildingId> {
        match self {
            Session::District { connected, .. } => *connected,
            Session::Rig { building } => Some(*building),
        }
    }
}

/// What a command answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub lines: Vec<String>,
    pub denied: bool,
}

impl Reply {
    fn say(lines: Vec<String>) -> Reply {
        Reply {
            lines,
            denied: false,
        }
    }

    fn one(line: impl Into<String>) -> Reply {
        Reply::say(vec![line.into()])
    }

    fn denied(have: Tier, need: Tier) -> Reply {
        Reply {
            lines: vec![
                format!(
                    "ACCESS DENIED · CLEARANCE: {} · REQUIRED: {}",
                    have.label(),
                    need.label()
                ),
                format!("IT TAKES {}", need.needs()),
            ],
            denied: true,
        }
    }
}

/// Your standing on a building: a backdoor makes you ADMIN, a card or a code USER.
pub fn tier_on(st: &AppState, b: BuildingId) -> Tier {
    if st.net.backdoors.contains(&b) {
        return Tier::Admin;
    }
    let card = st
        .character
        .inventory
        .iter()
        .filter_map(|s| credentials::parse_keycard(&s.item))
        .any(|(cb, _)| cb == b);
    let code = st.journal.codes().iter().any(|c| c.building == b);
    if card || code {
        Tier::User
    } else {
        Tier::Guest
    }
}

/// The tier a session acts with: the rig is at least USER on your own block.
pub fn tier_on_session(st: &AppState, s: &Session) -> Tier {
    match s {
        Session::District { connected, .. } => connected.map_or(Tier::Guest, |b| tier_on(st, b)),
        Session::Rig { building } => tier_on(st, *building).max(Tier::User),
    }
}

/// The ICE a construct runs, by its security.
pub fn ice_grade(security: u8) -> &'static str {
    if security >= BLACK_THRESHOLD {
        "black"
    } else if security >= GREY_THRESHOLD {
        "grey"
    } else if security >= ICE_THRESHOLD {
        "white"
    } else {
        "none"
    }
}

/// The district a street terminal's tile belongs to.
pub fn district_of_terminal(city: &City, k: StratumKind, p: Pos) -> Option<DistrictId> {
    city.tile(k, p)?.district
}

fn district_name(st: &AppState, d: DistrictId) -> String {
    st.city
        .as_ref()
        .map(|c| c.district(d).name.to_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

/// What the terminal calls a building: its name, or its kind and number when it has
/// none, so two apartment blocks never read alike.
pub fn building_label(st: &AppState, b: BuildingId) -> String {
    st.city
        .as_ref()
        .and_then(|c| c.building_opt(b))
        .map(|x| match &x.name {
            Some(n) => n.to_uppercase(),
            None => format!("{} {}", x.kind.label().to_uppercase(), b.index()),
        })
        .unwrap_or_else(|| "THE BUILDING".to_string())
}

fn building_name(st: &AppState, b: BuildingId) -> String {
    building_label(st, b)
}

fn security(city: &City, b: BuildingId) -> u8 {
    city.net
        .subnets
        .iter()
        .find(|s| s.owner == SubnetOwner::Building(b))
        .map(|s| s.security)
        .unwrap_or(0)
}

/// The prompt: where you are and your tier.
pub fn prompt(st: &AppState, s: &Session) -> String {
    match s {
        Session::District {
            district,
            connected: None,
        } => format!("{} NET > ", district_name(st, *district)),
        Session::District {
            connected: Some(b), ..
        } => format!("{} [{}] > ", building_name(st, *b), tier_on(st, *b).label()),
        Session::Rig { .. } => "RIG [HOME] > ".to_string(),
    }
}

/// What a terminal types when it powers on: the theme's boot with the place filled
/// in; the rig names itself first.
pub fn boot_lines(st: &AppState, s: &Session, look: &crate::ui::device::Look) -> Vec<String> {
    let place = match s {
        Session::District { district, .. } => district_name(st, *district),
        Session::Rig { building } => building_name(st, *building),
    };
    let mut lines: Vec<String> = look
        .style
        .boot
        .iter()
        .map(|l| l.replace("{PLACE}", &place))
        .collect();
    if matches!(s, Session::Rig { .. }) {
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines[0] = "HOME RIG · STRATA/OS".to_string();
    }
    lines
}

/// A verb, its gloss, the tier it needs, and whether it needs a connection.
struct Verb {
    name: &'static str,
    gloss: &'static str,
    tier: Tier,
    connected: bool,
}

const VERBS: &[Verb] = &[
    Verb {
        name: "HELP",
        gloss: "this list",
        tier: Tier::Guest,
        connected: false,
    },
    Verb {
        name: "SCAN",
        gloss: "the buildings on this subnet",
        tier: Tier::Guest,
        connected: false,
    },
    Verb {
        name: "CONNECT",
        gloss: "CONNECT <n|name>: pick a building",
        tier: Tier::Guest,
        connected: false,
    },
    Verb {
        name: "DISCONNECT",
        gloss: "back to the subnet",
        tier: Tier::Guest,
        connected: true,
    },
    Verb {
        name: "WHO",
        gloss: "who is inside now",
        tier: Tier::Guest,
        connected: true,
    },
    Verb {
        name: "PLAN",
        gloss: "load the ground floor plan",
        tier: Tier::Guest,
        connected: true,
    },
    Verb {
        name: "LOG",
        gloss: "the system log",
        tier: Tier::Guest,
        connected: true,
    },
    Verb {
        name: "DOOR",
        gloss: "DOOR UNLOCK: release the door locks",
        tier: Tier::User,
        connected: true,
    },
    Verb {
        name: "LIFT",
        gloss: "the lifts answer",
        tier: Tier::User,
        connected: true,
    },
    Verb {
        name: "CAM",
        gloss: "CAM LOOP: loop every camera",
        tier: Tier::User,
        connected: true,
    },
    Verb {
        name: "ALARM",
        gloss: "ALARM OFF: clear the alert",
        tier: Tier::Admin,
        connected: true,
    },
    Verb {
        name: "MAIL",
        gloss: "MAIL [n]: the inbox, or a message",
        tier: Tier::Guest,
        connected: false,
    },
    Verb {
        name: "JOURNAL",
        gloss: "what you know, by building",
        tier: Tier::Guest,
        connected: false,
    },
    Verb {
        name: "EXIT",
        gloss: "leave the terminal",
        tier: Tier::Guest,
        connected: false,
    },
];

fn verb_table(s: &Session) -> Vec<&'static Verb> {
    let rig = matches!(s, Session::Rig { .. });
    let connected = s.building().is_some();
    VERBS
        .iter()
        .filter(|v| match v.name {
            "CONNECT" | "DISCONNECT" => !rig && (v.name == "CONNECT" || connected),
            "MAIL" | "JOURNAL" => rig,
            _ => !v.connected || connected,
        })
        .collect()
}

/// The verbs this session offers, for HELP and completion.
pub fn verbs(st: &AppState, s: &Session) -> Vec<&'static str> {
    let _ = st;
    verb_table(s).iter().map(|v| v.name).collect()
}

/// Runs one typed line.
pub fn run(st: &mut AppState, s: &mut Session, line: &str) -> Reply {
    let upper = line.trim().to_uppercase();
    let mut words = upper.split_whitespace();
    let Some(verb) = words.next() else {
        return Reply::say(Vec::new());
    };
    let arg: Vec<&str> = words.collect();
    let rig = matches!(s, Session::Rig { .. });
    let known = VERBS.iter().find(|v| {
        v.name == verb
            && match v.name {
                "CONNECT" | "DISCONNECT" => !rig,
                "MAIL" | "JOURNAL" => rig,
                _ => true,
            }
    });
    let Some(v) = known else {
        return Reply::one("?SYNTAX ERROR");
    };
    if v.connected && s.building().is_none() {
        return not_connected();
    }
    let have = tier_on_session(st, s);
    if have < v.tier {
        return Reply::denied(have, v.tier);
    }
    match verb {
        "HELP" => help(st, s),
        "EXIT" => Reply::one("BYE"),
        "SCAN" => scan(st, s),
        "CONNECT" => connect(st, s, &arg),
        "DISCONNECT" => {
            if let Session::District { connected, .. } = s {
                *connected = None;
            }
            Reply::one("DISCONNECTED")
        }
        "WHO" => who(st, s),
        "PLAN" => plan(st, s),
        "LOG" => log(st, s),
        "DOOR" => door_unlock(st, s, &arg),
        "LIFT" => lift(st, s),
        "CAM" => cam_loop(st, s, &arg),
        "ALARM" => alarm_off(st, s, &arg),
        "MAIL" => mail(st, &arg),
        "JOURNAL" => journal(st),
        _ => Reply::one("?SYNTAX ERROR"),
    }
}

fn help(st: &AppState, s: &Session) -> Reply {
    let have = tier_on_session(st, s);
    Reply::say(
        verb_table(s)
            .iter()
            .map(|v| {
                let tag = if v.tier > have {
                    format!(" ({})", v.tier.label())
                } else {
                    String::new()
                };
                format!("{:<10} {}{tag}", v.name, v.gloss.to_uppercase())
            })
            .collect(),
    )
}

/// The buildings a session can see: the district's constructs, or the rig's block.
fn scannable(st: &AppState, s: &Session) -> Vec<BuildingId> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    match s {
        Session::Rig { building } => vec![*building],
        Session::District { district, .. } => city
            .buildings
            .iter()
            .filter(|b| b.district == *district && b.kind.is_closed())
            .filter(|b| {
                city.net
                    .nodes
                    .iter()
                    .any(|n| n.building == Some(b.id) && n.kind == NodeKind::Server)
            })
            .map(|b| b.id)
            .collect(),
    }
}

fn scan(st: &AppState, s: &Session) -> Reply {
    let Some(city) = st.city.as_ref() else {
        return Reply::one("NO CITY");
    };
    let list = scannable(st, s);
    if list.is_empty() {
        return Reply::one("NO CONSTRUCTS ON THIS SUBNET");
    }
    Reply::say(
        list.iter()
            .enumerate()
            .map(|(i, b)| {
                let sec = security(city, *b);
                format!(
                    "{:>2}  {:<22} {:<10} SEC {sec:>3}  ICE {:<5} {}",
                    i + 1,
                    building_name(st, *b),
                    city.building(*b).kind.label().to_uppercase(),
                    ice_grade(sec).to_uppercase(),
                    tier_on(st, *b).label()
                )
            })
            .collect(),
    )
}

fn connect(st: &AppState, s: &mut Session, arg: &[&str]) -> Reply {
    let list = scannable(st, s);
    let Session::District { connected, .. } = s else {
        return Reply::one("?SYNTAX ERROR");
    };
    if arg.is_empty() {
        return match connected {
            Some(b) => Reply::one(format!("CONNECTED TO {}", building_name(st, *b))),
            None => Reply::one("NOT CONNECTED · SCAN THEN CONNECT <N>"),
        };
    }
    let what = arg.join(" ");
    let numbered = |b: &BuildingId| list.iter().position(|x| x == b).map_or(0, |i| i + 1);
    let pick: Vec<BuildingId> = if let Ok(n) = what.parse::<usize>() {
        n.checked_sub(1)
            .and_then(|i| list.get(i))
            .map(|b| vec![*b])
            .unwrap_or_default()
    } else if let Some(exact) = list.iter().copied().find(|b| building_name(st, *b) == what) {
        vec![exact]
    } else {
        list.iter()
            .copied()
            .filter(|b| building_name(st, *b).starts_with(&what))
            .collect()
    };
    match pick.as_slice() {
        [] => Reply::one("NO SUCH BUILDING"),
        [b] => {
            *connected = Some(*b);
            Reply::one(format!(
                "CONNECTED TO {} · CLEARANCE {}",
                building_name(st, *b),
                tier_on(st, *b).label()
            ))
        }
        many => {
            let mut lines = vec!["AMBIGUOUS · DID YOU MEAN".to_string()];
            lines.extend(
                many.iter()
                    .map(|b| format!("{:>2}  {}", numbered(b), building_name(st, *b))),
            );
            Reply::say(lines)
        }
    }
}

fn not_connected() -> Reply {
    Reply::one("NOT CONNECTED · SCAN THEN CONNECT <N>")
}

fn who(st: &AppState, s: &Session) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    let Some(world) = st.world.as_ref() else {
        return Reply::one("NOBODY INSIDE");
    };
    let inside = world.inside(b);
    if inside.is_empty() {
        return Reply::one("NOBODY INSIDE");
    }
    Reply::say(
        inside
            .iter()
            .map(|o| {
                let p = &world.people()[o.id.0 as usize];
                let role = match o.role {
                    OccupantRole::Working => "AT WORK",
                    OccupantRole::Living => "AT HOME",
                    OccupantRole::Guest => "A GUEST",
                    OccupantRole::Visiting => "VISITING",
                };
                format!(
                    "{:<18} {:<12} {role}",
                    p.name.to_uppercase(),
                    p.profession.replace('_', " ").to_uppercase()
                )
            })
            .collect(),
    )
}

fn plan(st: &mut AppState, s: &Session) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    let Some(city) = st.city.as_ref() else {
        return Reply::one("NO CITY");
    };
    let Some(floor) = st.interiors.ensure(city, b, 0) else {
        return Reply::one("NO PLAN ON FILE");
    };
    let size = floor.tiles.size();
    let tiles: Vec<Pos> = floor.tiles.iter().map(|(p, _)| p).collect();
    let Some(player) = st.player.as_mut() else {
        return Reply::one("NO PLAN ON FILE");
    };
    player.remember_place(
        Place::Floor {
            building: b,
            level: 0,
        },
        size,
        tiles.into_iter(),
    );
    Reply::one("GROUND FLOOR PLAN LOADED")
}

fn stamp(minute: u64) -> String {
    let (h, m) = crate::ui::hud::clock_at(minute * sim::TICKS_PER_MINUTE);
    format!("{h:02}:{m:02}")
}

fn log(st: &AppState, s: &Session) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    let now = st.net.now(st);
    let mut lines: Vec<String> = st
        .net
        .hacks
        .iter()
        .rev()
        .filter(|h| h.building == b)
        .map(|h| {
            let target = match h.kind {
                Target::Node(k) => k.label().to_uppercase(),
                Target::Guard(_) => "ICE".to_string(),
                Target::Deck => "DECK".to_string(),
            };
            let op = match h.op {
                Op::Crack => "CRACKED",
                Op::Spoof => "SPOOFED",
                Op::Blind => "BLINDED",
                Op::Copy => "COPIED",
                Op::Backdoor => "BACKDOOR PLANTED",
                Op::Attack => "ATTACKED",
                _ => "TOUCHED",
            };
            let until = h
                .until
                .map(|u| format!("  (EXPIRES {})", stamp(u)))
                .unwrap_or_default();
            format!("{}  {target}  {op}{until}", stamp(h.at))
        })
        .collect();
    if st.net.alert(b, now) {
        lines.insert(0, format!("{}  TRACE ALERT", stamp(now)));
    }
    if lines.is_empty() {
        lines.push("NO ENTRIES".to_string());
    }
    Reply::say(lines)
}

fn nodes_of(st: &AppState, b: BuildingId, kind: NodeKind) -> Vec<citygen::NodeId> {
    st.city
        .as_ref()
        .map(|c| {
            c.net
                .nodes
                .iter()
                .filter(|n| n.building == Some(b) && n.kind == kind)
                .map(|n| n.id)
                .collect()
        })
        .unwrap_or_default()
}

fn door_unlock(st: &mut AppState, s: &Session, arg: &[&str]) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    if arg.first() != Some(&"UNLOCK") {
        return Reply::one("?SYNTAX ERROR · DOOR UNLOCK");
    }
    let Some(lock) = nodes_of(st, b, NodeKind::DoorLock).first().copied() else {
        return Reply::one("NO DOOR CONTROLLER");
    };
    let now = st.net.now(st);
    if st.net.alert(b, now) {
        return Reply::one("LOCKDOWN IN FORCE · THE LOCKS HOLD UNTIL THE ALERT ENDS");
    }
    st.net.land(
        b,
        HackTarget::Node(lock),
        Target::Node(NodeKind::DoorLock),
        Op::Spoof,
        now,
    );
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s locked doors open for an hour"));
    Reply::one("DOOR LOCKS RELEASED · 60 MIN")
}

fn lift(st: &mut AppState, s: &Session) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    let Some(ctl) = nodes_of(st, b, NodeKind::LiftCtrl).first().copied() else {
        return Reply::one("NO LIFT CONTROLLER");
    };
    let now = st.net.now(st);
    st.net.land(
        b,
        HackTarget::Node(ctl),
        Target::Node(NodeKind::LiftCtrl),
        Op::Spoof,
        now,
    );
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s lifts answer you"));
    Reply::one("LIFTS ANSWERING · 60 MIN")
}

fn cam_loop(st: &mut AppState, s: &Session, arg: &[&str]) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    if arg.first() != Some(&"LOOP") {
        return Reply::one("?SYNTAX ERROR · CAM LOOP");
    }
    let cams = nodes_of(st, b, NodeKind::Camera);
    if cams.is_empty() {
        return Reply::one("NO CAMERAS");
    }
    let now = st.net.now(st);
    for c in &cams {
        st.net.land(
            b,
            HackTarget::Node(*c),
            Target::Node(NodeKind::Camera),
            Op::Blind,
            now,
        );
    }
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s cameras loop for an hour"));
    Reply::one(format!("{} CAMERAS LOOPED · 60 MIN", cams.len()))
}

fn alarm_off(st: &mut AppState, s: &Session, arg: &[&str]) -> Reply {
    let Some(b) = s.building() else {
        return not_connected();
    };
    if arg.first() != Some(&"OFF") {
        return Reply::one("?SYNTAX ERROR · ALARM OFF");
    }
    let now = st.net.now(st);
    if !st.net.alert(b, now) {
        return Reply::one("NO ALARM");
    }
    st.net.trace(b).alert_until = Some(now);
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s alarm falls silent"));
    Reply::one("ALARM CLEARED")
}

fn mail(st: &mut AppState, arg: &[&str]) -> Reply {
    if st.journal.mails.is_empty() {
        return Reply::one("NO MAIL");
    }
    match arg.first().and_then(|a| a.parse::<usize>().ok()) {
        None => Reply::say(
            st.journal
                .mails
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    format!(
                        "{:>2}  {} {:<10} {}",
                        i + 1,
                        if m.read { " " } else { "*" },
                        m.from.to_uppercase(),
                        m.subject.to_uppercase()
                    )
                })
                .collect(),
        ),
        Some(n) => {
            let Some(i) = n.checked_sub(1).filter(|i| *i < st.journal.mails.len()) else {
                return Reply::one("NO SUCH MESSAGE");
            };
            let m = &st.journal.mails[i];
            let mut lines = vec![
                format!("FROM: {}", m.from.to_uppercase()),
                format!("SUBJECT: {}", m.subject.to_uppercase()),
                String::new(),
            ];
            lines.extend(m.body.iter().cloned());
            credentials::mail_read(st, i);
            Reply::say(lines)
        }
    }
}

fn journal(st: &AppState) -> Reply {
    let groups = credentials::entries(st);
    if groups.is_empty() {
        return Reply::one("NOTHING KNOWN YET");
    }
    let mut lines = Vec::new();
    for (_, header, es) in groups {
        lines.push(header);
        for e in es {
            let (text, _) = crate::messages::parse_marks(&e.text);
            lines.push(format!("  {text}"));
        }
    }
    Reply::say(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use citygen::{BuildingKind, GenParams, Generator, NodeKind, SizePreset};
    use game::{HackTarget, Op, Target};

    /// A city, a residential building with people home, and its district.
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
        (st, b, d)
    }

    fn district(d: citygen::DistrictId) -> Session {
        Session::District {
            district: d,
            connected: None,
        }
    }

    fn connected(d: citygen::DistrictId, b: BuildingId) -> Session {
        Session::District {
            district: d,
            connected: Some(b),
        }
    }

    fn lines(st: &mut AppState, s: &mut Session, line: &str) -> String {
        run(st, s, line).lines.join("\n")
    }

    #[test]
    fn tiers_come_from_what_you_hold() {
        let (mut st, b, d) = state();
        crate::credentials::prepare(&mut st, b);
        assert_eq!(tier_on(&st, b), Tier::Guest);
        let h = st
            .interiors
            .homes_of(b)
            .unwrap()
            .households()
            .next()
            .unwrap()
            .0;
        st.character.give(crate::credentials::keycard_id(b, h), 1);
        assert_eq!(tier_on(&st, b), Tier::User);
        st.character.inventory.clear();
        crate::credentials::copy_codes(&mut st, b, 1);
        assert_eq!(tier_on(&st, b), Tier::User);
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
        let now = st.net.now(&st);
        st.net.land(
            b,
            HackTarget::Node(server),
            Target::Node(NodeKind::Server),
            Op::Backdoor,
            now,
        );
        assert_eq!(tier_on(&st, b), Tier::Admin);
        assert!(Tier::Guest < Tier::User && Tier::User < Tier::Admin);
        let name = building_label(&st, b);
        assert_eq!(prompt(&st, &connected(d, b)), format!("{name} [ADMIN] > "));
        let dn = st.city.as_ref().unwrap().district(d).name.to_uppercase();
        assert_eq!(prompt(&st, &district(d)), format!("{dn} NET > "));
        assert_eq!(prompt(&st, &Session::Rig { building: b }), "RIG [HOME] > ");
        assert_eq!(
            tier_on_session(&st, &Session::Rig { building: b }),
            Tier::Admin
        );
    }

    #[test]
    fn scan_lists_constructs_with_grades_and_connect_picks_one() {
        let (mut st, b, d) = state();
        let mut s = district(d);
        let scan = lines(&mut st, &mut s, "scan");
        let name = building_label(&st, b);
        let row = scan
            .lines()
            .find(|l| l.contains(&name))
            .unwrap_or_else(|| panic!("{scan}"));
        assert!(
            row.contains("SEC ") && row.contains("ICE ") && row.contains("GUEST"),
            "{row}"
        );
        let n: usize = row.split_whitespace().next().unwrap().parse().unwrap();
        assert!(lines(&mut st, &mut s, &format!("connect {n}")).contains("CONNECTED"));
        assert!(matches!(s, Session::District { connected: Some(x), .. } if x == b));
        assert!(lines(&mut st, &mut s, "disconnect").contains("DISCONNECTED"));
        assert!(matches!(
            s,
            Session::District {
                connected: None,
                ..
            }
        ));
        let prefix: String = name.chars().take(4).collect();
        let reply = lines(&mut st, &mut s, &format!("connect {prefix}"));
        let matching = scan
            .lines()
            .filter(|l| l.contains(&format!("  {prefix}")))
            .count();
        if matching == 1 {
            assert!(reply.contains("CONNECTED"), "{reply}");
        } else {
            assert!(reply.contains("AMBIGUOUS"), "{reply}");
            assert!(matches!(
                s,
                Session::District {
                    connected: None,
                    ..
                }
            ));
        }
        assert!(lines(&mut st, &mut s, "connect zzzzzz").contains("NO SUCH BUILDING"));
        assert!(lines(&mut st, &mut s, "who").contains("NOT CONNECTED"));
        assert_eq!(ice_grade(0), "none");
        assert_eq!(ice_grade(160), "white");
        assert_eq!(ice_grade(200), "grey");
        assert_eq!(ice_grade(230), "black");
    }

    #[test]
    fn who_plan_and_log_read_the_building() {
        let (mut st, b, d) = state();
        let mut s = connected(d, b);
        let who = lines(&mut st, &mut s, "who");
        let world = st.world.as_ref().unwrap();
        let home = world
            .inside(b)
            .into_iter()
            .find(|o| o.role == sim::OccupantRole::Living)
            .expect("a resident home at ten at night");
        let name = world.people()[home.id.0 as usize].name.to_uppercase();
        assert!(who.contains(&name) && who.contains("AT HOME"), "{who}");
        assert!(lines(&mut st, &mut s, "plan").contains("GROUND FLOOR PLAN LOADED"));
        let floor_place = citygen::Place::Floor {
            building: b,
            level: 0,
        };
        let vis = st
            .player
            .as_ref()
            .unwrap()
            .vis_at(floor_place)
            .expect("a plan");
        assert!(
            vis.iter()
                .any(|(_, v)| *v == crate::walker::Visibility::Remembered)
        );
        assert!(lines(&mut st, &mut s, "log").contains("NO ENTRIES"));
        let lock = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .find(|n| n.building == Some(b) && n.kind == NodeKind::DoorLock)
            .map(|n| n.id)
            .unwrap();
        let now = st.net.now(&st);
        st.net.land(
            b,
            HackTarget::Node(lock),
            Target::Node(NodeKind::DoorLock),
            Op::Crack,
            now,
        );
        let log = lines(&mut st, &mut s, "log");
        assert!(
            log.contains("DOOR LOCK") && log.contains("CRACKED"),
            "{log}"
        );
    }

    #[test]
    fn world_commands_need_user_and_land_hacks() {
        let (mut st, b, d) = state();
        crate::credentials::prepare(&mut st, b);
        let mut s = connected(d, b);
        let denied = run(&mut st, &mut s, "door unlock");
        assert!(denied.denied);
        assert_eq!(
            denied.lines[0],
            "ACCESS DENIED · CLEARANCE: GUEST · REQUIRED: USER"
        );
        crate::credentials::copy_codes(&mut st, b, 1);
        let now = st.net.now(&st);
        assert!(!st.net.unlocked(b, now));
        assert!(lines(&mut st, &mut s, "door unlock").contains("DOOR LOCKS RELEASED"));
        assert!(st.net.unlocked(b, now));
        let has_lift = !nodes_of(&st, b, NodeKind::LiftCtrl).is_empty();
        let reply = lines(&mut st, &mut s, "lift");
        assert!(
            reply.contains(if has_lift {
                "LIFTS ANSWERING"
            } else {
                "NO LIFT CONTROLLER"
            }),
            "{reply}"
        );
        let cams: Vec<_> = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .filter(|n| n.building == Some(b) && n.kind == NodeKind::Camera)
            .map(|n| n.id)
            .collect();
        let reply = lines(&mut st, &mut s, "cam loop");
        if cams.is_empty() {
            assert!(reply.contains("NO CAMERAS"), "{reply}");
        } else {
            assert!(reply.contains("LOOPED"), "{reply}");
            assert!(cams.iter().all(|c| st.net.blinded(*c, now)));
        }
        let denied = run(&mut st, &mut s, "alarm off");
        assert_eq!(
            denied.lines[0],
            "ACCESS DENIED · CLEARANCE: USER · REQUIRED: ADMIN"
        );
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
            HackTarget::Node(server),
            Target::Node(NodeKind::Server),
            Op::Backdoor,
            now,
        );
        assert!(lines(&mut st, &mut s, "alarm off").contains("NO ALARM"));
        st.net.trace(b).alert_until = Some(now + 30);
        assert!(st.net.alert(b, now));
        assert!(lines(&mut st, &mut s, "alarm off").contains("ALARM CLEARED"));
        assert!(!st.net.alert(b, now));
    }

    #[test]
    fn syntax_help_exit_and_boot() {
        let (mut st, b, d) = state();
        let mut s = district(d);
        assert!(lines(&mut st, &mut s, "frobnicate").contains("?SYNTAX ERROR"));
        let help = lines(&mut st, &mut s, "help");
        assert!(
            help.contains("SCAN") && help.contains("CONNECT") && !help.contains("DOOR"),
            "{help}"
        );
        let mut c = connected(d, b);
        let help = lines(&mut st, &mut c, "help");
        assert!(
            help.contains("WHO") && help.contains("DOOR") && help.contains("ALARM"),
            "{help}"
        );
        assert!(verbs(&st, &c).contains(&"PLAN"));
        assert!(!verbs(&st, &s).contains(&"WHO"));
        assert_eq!(lines(&mut st, &mut s, "exit"), "BYE");
        let muni = crate::ui::device::Look::named(&st, "municipal", Some(d), 1);
        let boot = boot_lines(&st, &s, &muni);
        assert_eq!(boot.len(), 3);
        assert!(
            boot[0].contains("MUNI-OS") && boot[2] == "READY",
            "{boot:?}"
        );
        let dn = st.city.as_ref().unwrap().district(d).name.to_uppercase();
        assert!(boot[0].contains(&dn), "{boot:?}");
        let street = crate::ui::device::Look::named(&st, "street", Some(d), 1);
        let boot = boot_lines(&st, &s, &street);
        assert_eq!(boot[2], "REDY");
        let rig = boot_lines(&st, &Session::Rig { building: b }, &street);
        assert!(
            rig[0].contains("HOME RIG") && rig[0].contains("STRATA/OS"),
            "{rig:?}"
        );
        let city = st.city.as_ref().unwrap();
        let term = city
            .net
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Terminal && n.building.is_none())
            .unwrap();
        let (k, p) = term.anchor;
        assert_eq!(
            district_of_terminal(city, k, p),
            city.tile(k, p).unwrap().district
        );
    }

    #[test]
    fn connect_takes_whole_names_exact_first_and_labels_unnamed_buildings_apart() {
        let (mut st, _b, d) = state();
        let mut s = district(d);
        let scan = lines(&mut st, &mut s, "scan");
        let names: Vec<String> = scan
            .lines()
            .map(|l| l[4..26].trim_end().to_string())
            .collect();
        let distinct: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(
            distinct.len(),
            names.len(),
            "every row has its own label: {scan}"
        );
        // The whole name connects, spaces and all.
        let full = names[0].clone();
        let reply = lines(&mut st, &mut s, &format!("connect {full}"));
        assert!(reply.contains("CONNECTED"), "{reply}");
        // An exact name wins over longer names it prefixes.
        let short = names
            .iter()
            .find(|n| names.iter().any(|m| m != *n && m.starts_with(n.as_str())));
        if let Some(short) = short {
            let reply = lines(&mut st, &mut s, &format!("connect {short}"));
            assert!(reply.contains(&format!("CONNECTED TO {short}")), "{reply}");
        }
        // Ambiguity lists numbered candidates.
        let first: String = names[0].chars().take(1).collect();
        let hits = names.iter().filter(|n| n.starts_with(&first)).count();
        if hits > 1 {
            let reply = lines(&mut st, &mut s, &format!("connect {first}"));
            assert!(reply.contains("AMBIGUOUS"), "{reply}");
            assert!(
                reply.lines().skip(1).all(|l| l
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())),
                "{reply}"
            );
        }
    }

    #[test]
    fn door_unlock_during_an_alert_tells_the_truth() {
        let (mut st, b, d) = state();
        crate::credentials::prepare(&mut st, b);
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut s = connected(d, b);
        let now = st.net.now(&st);
        st.net.trace(b).alert_until = Some(now + 30);
        let reply = lines(&mut st, &mut s, "door unlock");
        assert!(reply.contains("LOCKDOWN IN FORCE"), "{reply}");
        assert!(!reply.contains("60 MIN"));
    }
}
