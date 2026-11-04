//! The world behind the Access Terminal: your standing on a building, the constructs a
//! district terminal sees, and the actions and states the HELIX/OS devices and
//! programs call. Plain functions over the app state that answer in text.

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
}

/// Your standing on a building: a Net backdoor or a cracked server makes you ADMIN, a key, a card or a code USER.
pub fn tier_on(st: &AppState, b: BuildingId) -> Tier {
    let now = st.net.now(st);
    if st.backdoors.root_on(b) || st.net.server_cracked(b, now) {
        return Tier::Admin;
    }
    let key = st
        .backdoors
        .live(b)
        .is_some_and(|d| matches!(d.kind, crate::backdoors::Kind::Key { .. }));
    let card = st
        .character
        .inventory
        .iter()
        .filter_map(|s| credentials::parse_keycard(&s.item))
        .any(|(cb, _)| cb == b);
    let code = st.journal.codes().iter().any(|c| c.building == b);
    // A terminal of the building held from the Net: a user's seat for the hour.
    let terminal = crate::holds::terminal_held(st, crate::owners::Owner::Building(b));
    if key || card || code || terminal {
        Tier::User
    } else {
        Tier::Guest
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
            None => format!(
                "{} {}",
                citygen::kind_label(st.city.as_ref().unwrap(), b).to_uppercase(),
                b.index()
            ),
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

pub fn security_of(st: &AppState, b: BuildingId) -> u8 {
    st.city.as_ref().map_or(0, |c| security(c, b))
}

// ---- the world actions: what the HELIX/OS devices and programs call ----

/// The buildings a district terminal can see: its closed buildings with a server.
pub fn scannable(st: &AppState, district: DistrictId) -> Vec<BuildingId> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    city.buildings
        .iter()
        .filter(|b| b.district == district && b.kind.is_closed())
        .filter(|b| {
            city.net
                .nodes
                .iter()
                .any(|n| n.building == Some(b.id) && n.kind == NodeKind::Server)
        })
        .map(|b| b.id)
        .collect()
}

/// One row of a scan: its number, the building, its security and ICE, and your standing.
pub fn scan_row(st: &AppState, i: usize, b: BuildingId) -> String {
    let Some(city) = st.city.as_ref() else {
        return String::new();
    };
    let sec = security(city, b);
    format!(
        "{:>2}  {:<22} {:<10} SEC {sec:>3}  ICE {:<5} {}",
        i + 1,
        building_name(st, b),
        citygen::kind_label(city, b).to_uppercase(),
        ice_grade(sec).to_uppercase(),
        tier_on(st, b).label()
    )
}

fn stamp(minute: u64) -> String {
    let (h, m) = crate::ui::hud::clock_at(minute * sim::TICKS_PER_MINUTE);
    format!("{h:02}:{m:02}")
}

/// The auth log: the hacks on this building, newest first.
pub fn log_lines(st: &AppState, b: BuildingId) -> Vec<String> {
    let now = st.net.now(st);
    let mut rows = Vec::new();
    if st
        .backdoors
        .list
        .iter()
        .any(|d| d.building == b && d.state == crate::backdoors::State::Honeypot)
    {
        rows.push(format!("{}  audit  key flagged · monitoring", stamp(now)));
    }
    for (at, lb, user) in st.backdoors.logins.iter().rev() {
        if *lb == b {
            rows.push(format!("{}  login  {user} via key", stamp(*at)));
        }
    }
    rows.extend(hack_rows(st, b));
    rows
}

/// The rig's line on a backdoored host: `LINE  root  STABLE  planted day 2 03:12`.
pub fn line_row(st: &AppState, b: BuildingId) -> Option<String> {
    let d = st.backdoors.list.iter().rev().find(|d| d.building == b)?;
    let now = st.net.now(st);
    Some(format!(
        "LINE  {}  {}  planted {}",
        d.user_label(),
        d.health(now).label(),
        crate::credentials::stamp(d.planted)
    ))
}

/// What `route` and the rig's net.log say: the route, the trace, the lines held.
pub fn rig_net_lines(st: &AppState) -> Vec<String> {
    let now = st.net.now(st);
    let name = |b: BuildingId| crate::helix::hosts::building_hostname(st, b);
    let mut out = Vec::new();
    match st.backdoors.route.as_ref() {
        Some(r) => {
            let names: Vec<String> = r.relays.iter().map(|b| name(*b)).collect();
            out.push(format!("ROUTE  {}", crate::backdoors::route_line(&names)));
            for b in &r.relays {
                if let Some(d) = st.backdoors.list.iter().rev().find(|d| d.building == *b) {
                    out.push(format!(
                        "  {:<16} {:<10} {}",
                        name(*b),
                        d.user_label(),
                        d.health(now).label()
                    ));
                }
            }
        }
        None => out.push("NO ROUTE · you are home".to_string()),
    }
    out.push(
        st.backdoors
            .trace_line(now, &name)
            .unwrap_or_else(|| "TRACE  none".to_string()),
    );
    out.push(format!(
        "LINES  {} of {}",
        st.backdoors.slots_used(),
        crate::backdoors::SLOTS
    ));
    out
}

fn hack_rows(st: &AppState, b: BuildingId) -> Vec<String> {
    st.net
        .hacks
        .iter()
        .rev()
        .filter(|h| h.building == b)
        .map(|h| {
            let target = match h.kind {
                Target::Node(k) => k.label().to_lowercase(),
                Target::Guard(_) => "ice".to_string(),
                Target::Deck => "deck".to_string(),
            };
            let op = match h.op {
                Op::Crack => "cracked",
                Op::Spoof => "spoofed",
                Op::Blind => "blinded",
                Op::Copy => "copied",
                Op::Backdoor => "backdoor planted",
                Op::Attack => "attacked",
                _ => "touched",
            };
            let until = h
                .until
                .map(|u| format!("  (expires {})", stamp(u)))
                .unwrap_or_default();
            format!("{}  {target}  {op}{until}", stamp(h.at))
        })
        .collect()
}

/// The trace: heat and the alert.
pub fn trace_lines(st: &AppState, b: BuildingId) -> Vec<String> {
    let now = st.net.now(st);
    let trace = st.net.traces.get(&b);
    let heat = trace.map_or(0, |t| t.heat);
    let alert = trace
        .and_then(|t| t.alert_until)
        .filter(|u| now < *u)
        .map_or_else(
            || "quiet".to_string(),
            |u| format!("alert until {}", stamp(u)),
        );
    vec![format!("heat {heat}"), alert]
}

/// Who is inside: a name and what they are doing there.
pub fn who_lines(st: &AppState, b: BuildingId) -> Vec<String> {
    let Some(world) = st.world.as_ref() else {
        return Vec::new();
    };
    world
        .inside(b)
        .iter()
        .map(|o| {
            let p = &world.people()[o.id.0 as usize];
            let role = match o.role {
                OccupantRole::Working => "at work",
                OccupantRole::Living => "at home",
                OccupantRole::Guest => "a guest",
                OccupantRole::Visiting => "visiting",
            };
            format!("{:<16} {role}", crate::helix::hosts::slug(&p.name))
        })
        .collect()
}

/// Loads the ground floor into the player's map.
pub fn load_plan(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let Some(city) = st.city.as_ref() else {
        return Err("NO CITY".to_string());
    };
    let Some(floor) = st.interiors.ensure(city, b, 0) else {
        return Err("NO PLAN ON FILE".to_string());
    };
    let size = floor.tiles.size();
    let tiles: Vec<Pos> = floor.tiles.iter().map(|(p, _)| p).collect();
    let Some(player) = st.player.as_mut() else {
        return Err("NO PLAN ON FILE".to_string());
    };
    player.remember_place(
        Place::Floor {
            building: b,
            level: 0,
        },
        size,
        tiles.into_iter(),
    );
    Ok("GROUND FLOOR PLAN LOADED".to_string())
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

pub fn release_doors(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let Some(lock) = nodes_of(st, b, NodeKind::DoorLock).first().copied() else {
        return Err("NO DOOR CONTROLLER".to_string());
    };
    let now = st.net.now(st);
    if st.net.alert(b, now) {
        return Err("LOCKDOWN IN FORCE · THE LOCKS HOLD UNTIL THE ALERT ENDS".to_string());
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
    Ok("DOOR LOCKS RELEASED · 60 MIN".to_string())
}

pub fn answer_lifts(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let Some(ctl) = nodes_of(st, b, NodeKind::LiftCtrl).first().copied() else {
        return Err("NO LIFT CONTROLLER".to_string());
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
    Ok("LIFTS ANSWERING · 60 MIN".to_string())
}

/// Loops every camera of the building.
pub fn loop_cams(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    loop_some(st, b, None)
}

/// Loops camera `i` alone.
pub fn loop_cam(st: &mut AppState, b: BuildingId, i: usize) -> Result<String, String> {
    loop_some(st, b, Some(i))
}

fn loop_some(st: &mut AppState, b: BuildingId, only: Option<usize>) -> Result<String, String> {
    let mut cams = nodes_of(st, b, NodeKind::Camera);
    if let Some(i) = only {
        cams = cams.get(i).copied().into_iter().collect();
    }
    if cams.is_empty() {
        return Err("NO CAMERAS".to_string());
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
    let n = cams.len();
    Ok(format!(
        "{n} CAMERA{} LOOPED · 60 MIN",
        if n == 1 { "" } else { "S" }
    ))
}

pub fn alarm_off(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let now = st.net.now(st);
    if !st.net.alert(b, now) {
        return Err("NO ALARM".to_string());
    }
    st.net.trace(b).alert_until = Some(now);
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s alarm falls silent"));
    Ok("ALARM CLEARED".to_string())
}

/// When the latest live hack of this op on this kind of node runs out.
fn live_until(st: &AppState, b: BuildingId, kind: NodeKind, ops: &[Op], now: u64) -> Option<u64> {
    st.net
        .hacks
        .iter()
        .filter(|h| h.building == b && h.kind == Target::Node(kind) && ops.contains(&h.op))
        .filter(|h| h.until.is_none_or(|u| now < u))
        .filter_map(|h| h.until)
        .max()
}

pub fn door_state(st: &AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    if st.net.alert(b, now) {
        "lockdown".to_string()
    } else if st.net.unlocked(b, now) {
        match live_until(st, b, NodeKind::DoorLock, &[Op::Crack, Op::Spoof], now) {
            Some(u) => format!("locks: released (until {})", stamp(u)),
            None => "locks: released".to_string(),
        }
    } else {
        "locks: engaged".to_string()
    }
}

pub fn lift_state(st: &AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    if !st.net.lifts_answer(b, now) {
        "lifts: stopped".to_string()
    } else if live_until(st, b, NodeKind::LiftCtrl, &[Op::Spoof], now).is_some() {
        "lifts: answering".to_string()
    } else {
        "lifts: normal".to_string()
    }
}

pub fn cam_state(st: &AppState, b: BuildingId, i: usize) -> String {
    let now = st.net.now(st);
    let Some(node) = nodes_of(st, b, NodeKind::Camera).get(i).copied() else {
        return format!("cam{i}: absent");
    };
    if !st.net.blinded(node, now) {
        return format!("cam{i}: live");
    }
    let until = st
        .net
        .hacks
        .iter()
        .filter(|h| h.target == HackTarget::Node(node) && h.op == Op::Blind)
        .filter(|h| h.until.is_none_or(|u| now < u))
        .filter_map(|h| h.until)
        .max();
    match until {
        Some(u) => format!("cam{i}: looped (until {})", stamp(u)),
        None => format!("cam{i}: looped"),
    }
}

pub fn alarm_state(st: &AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    if st.net.alert(b, now) {
        "alarm: RINGING".to_string()
    } else {
        "alarm: quiet".to_string()
    }
}

/// A mail's time of day, `hh:mm`.
pub fn mail_time(m: &credentials::Mail) -> String {
    let t = crate::jobs::day_minute(m.at);
    format!("{:02}:{:02}", t / 60, t % 60)
}

/// The rule above and below a printed mail's body.
const MAIL_RULE: &str = "────────────────────────────────────────────";

/// A mail's text, read: it is marked read and its story step ticks. A head block, the
/// body between two rules, and how many unread are left.
pub fn mail_open(st: &mut AppState, i: usize) -> Result<Vec<String>, String> {
    let n = st.journal.mails.len();
    let Some(m) = st.journal.mails.get(i) else {
        return Err("NO SUCH MESSAGE".to_string());
    };
    let new = if m.read { "" } else { " · NEW" };
    let mut lines = vec![
        format!("MAIL {} OF {n}{new}", i + 1),
        format!("FROM     {}", m.from.to_uppercase()),
        format!("SUBJECT  {}", m.subject.to_uppercase()),
        format!("TIME     {}", mail_time(m)),
        MAIL_RULE.to_string(),
    ];
    lines.extend(m.body.iter().cloned());
    lines.push(MAIL_RULE.to_string());
    credentials::mail_read(st, i);
    let left = st.journal.mails.iter().filter(|m| !m.read).count();
    lines.push(if left == 0 {
        "NO NEW MAIL".to_string()
    } else {
        format!("{left} NEW LEFT · mail")
    });
    Ok(lines)
}

/// What you know, by building, with the marks stripped.
pub fn journal_lines(st: &AppState) -> Vec<String> {
    let groups = credentials::entries(st);
    let mut lines = credentials::extra_lines(st);
    if groups.is_empty() && lines.is_empty() {
        return vec!["NOTHING KNOWN YET".to_string()];
    }
    for (_, header, es) in groups {
        lines.push(header);
        for e in es {
            let (text, _) = crate::messages::parse_marks(&e.text);
            lines.push(format!("  {text}"));
        }
    }
    lines
}

/// What a HELIX/OS host types when it powers on: the theme's boot with the OS and the
/// place filled in.
pub fn boot_lines(
    st: &AppState,
    host: crate::helix::shell::HostId,
    look: &crate::ui::device::Look,
) -> Vec<String> {
    let place = place_label(st, host);
    look.style
        .boot
        .iter()
        .map(|l| {
            let mut l = l.replace("{PLACE}", &place).replace("{OS}", &look.os());
            if l.contains("{HOST}") {
                l = l.replace("{HOST}", &crate::helix::hosts::hostname(st, host));
            }
            if l.contains("{USER}") {
                l = l.replace("{USER}", &crate::helix::hosts::user_for(st, host));
            }
            l
        })
        .collect()
}

/// What a host's machine calls where it stands: the district, the building, your block.
pub fn place_label(st: &AppState, host: crate::helix::shell::HostId) -> String {
    use crate::helix::shell::HostId;
    match host {
        HostId::District(d) => district_name(st, d),
        HostId::Building(b) => building_name(st, b),
        HostId::Rig => st.home.as_ref().map_or_else(
            || "ZENBO".to_string(),
            |h| building_name(st, h.home.building),
        ),
    }
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

    #[test]
    fn a_cracked_server_is_root_for_its_hour() {
        let (mut st, b, _) = state();
        assert_eq!(tier_on(&st, b), Tier::Guest);
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
            .expect("a server");
        st.net.land(
            b,
            HackTarget::Node(server),
            Target::Node(NodeKind::Server),
            Op::Crack,
            now,
        );
        assert!(st.net.server_cracked(b, now));
        assert_eq!(tier_on(&st, b), Tier::Admin);
        assert_eq!(
            crate::helix::hosts::user_for(&st, crate::helix::shell::HostId::Building(b)),
            "root"
        );
        // the hour runs out: guest again
        let roots = st.backdoors.roots();
        st.net
            .minute(now + game::HACK_MINUTES, None, &mut st.constructs, &roots);
        assert!(!st.net.server_cracked(b, now + game::HACK_MINUTES));
        assert_eq!(tier_on(&st, b), Tier::Guest);
        // a cracked door lock is not a cracked server
        let lock = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .find(|n| n.building == Some(b) && n.kind == NodeKind::DoorLock)
            .map(|n| n.id);
        if let Some(lock) = lock {
            let now = st.net.now(&st);
            st.net.land(
                b,
                HackTarget::Node(lock),
                Target::Node(NodeKind::DoorLock),
                Op::Crack,
                now,
            );
            assert_eq!(tier_on(&st, b), Tier::Guest);
        }
    }

    #[test]
    fn a_key_keeps_the_tier_and_the_user_after_the_card_is_gone() {
        use crate::helix::hosts::user_for;
        use crate::helix::shell::HostId;
        let (mut st, b, _) = state();
        crate::credentials::prepare(&mut st, b);
        let h = st
            .interiors
            .homes_of(b)
            .unwrap()
            .households()
            .next()
            .unwrap()
            .0;
        st.character.give(crate::credentials::keycard_id(b, h), 1);
        let user = user_for(&st, HostId::Building(b));
        assert_ne!(user, "guest");
        st.backdoors.plant_key(b, &user, 100, 0, 0);
        st.character
            .inventory
            .retain(|s| crate::credentials::parse_keycard(&s.item).is_none());
        assert_eq!(tier_on(&st, b), Tier::User);
        assert_eq!(user_for(&st, HostId::Building(b)), user);
        st.backdoors.burn(b, crate::backdoors::Why::You, 1);
        assert_eq!(tier_on(&st, b), Tier::Guest);
        st.backdoors.plant_net(b, 1, 0, 2);
        assert_eq!(tier_on(&st, b), Tier::Admin);
        assert_eq!(user_for(&st, HostId::Building(b)), "root");
    }

    #[test]
    fn tiers_come_from_what_you_hold() {
        let (mut st, b, _) = state();
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
        st.backdoors.plant_net(b, 1, 0, now);
        assert_eq!(tier_on(&st, b), Tier::Admin);
        assert!(Tier::Guest < Tier::User && Tier::User < Tier::Admin);
    }

    #[test]
    fn world_actions_land_hacks_and_the_states_read_them_back() {
        let (mut st, b, d) = state();
        crate::credentials::prepare(&mut st, b);
        let now = st.net.now(&st);
        assert_eq!(door_state(&st, b), "locks: engaged");
        assert_eq!(lift_state(&st, b), "lifts: normal");
        assert_eq!(alarm_state(&st, b), "alarm: quiet");
        assert_eq!(trace_lines(&st, b), vec!["heat 0", "quiet"]);
        assert!(log_lines(&st, b).is_empty());
        assert_eq!(
            release_doors(&mut st, b).as_deref(),
            Ok("DOOR LOCKS RELEASED · 60 MIN")
        );
        assert!(st.net.unlocked(b, now));
        assert!(door_state(&st, b).starts_with("locks: released (until "));
        assert!(log_lines(&st, b)[0].contains("door lock  spoofed"));
        match answer_lifts(&mut st, b) {
            Ok(r) => {
                assert_eq!(r, "LIFTS ANSWERING · 60 MIN");
                assert_eq!(lift_state(&st, b), "lifts: answering");
            }
            Err(e) => assert_eq!(e, "NO LIFT CONTROLLER"),
        }
        let cams = nodes_of(&st, b, NodeKind::Camera);
        match loop_cams(&mut st, b) {
            Ok(r) => {
                assert!(r.ends_with("LOOPED · 60 MIN"), "{r}");
                assert!(cams.iter().all(|c| st.net.blinded(*c, now)));
                assert!(cam_state(&st, b, 0).starts_with("cam0: looped (until "));
            }
            Err(e) => assert_eq!((e.as_str(), cams.len()), ("NO CAMERAS", 0)),
        }
        assert_eq!(alarm_off(&mut st, b), Err("NO ALARM".to_string()));
        // an alert: the locks hold, the states say so, the alarm can be cleared
        st.net.trace(b).alert_until = Some(now + 30);
        assert_eq!(
            release_doors(&mut st, b).unwrap_err(),
            "LOCKDOWN IN FORCE · THE LOCKS HOLD UNTIL THE ALERT ENDS"
        );
        assert_eq!(door_state(&st, b), "lockdown");
        assert_eq!(alarm_state(&st, b), "alarm: RINGING");
        assert!(trace_lines(&st, b)[1].starts_with("alert until "));
        assert_eq!(alarm_off(&mut st, b).as_deref(), Ok("ALARM CLEARED"));
        assert!(!st.net.alert(b, now));
        // the district lists the building and its row carries grade and standing
        let list = scannable(&st, d);
        let i = list.iter().position(|x| *x == b).expect("listed");
        let row = scan_row(&st, i, b);
        assert!(
            row.contains(&building_label(&st, b)) && row.contains("ICE "),
            "{row}"
        );
        assert_eq!(
            load_plan(&mut st, b).as_deref(),
            Ok("GROUND FLOOR PLAN LOADED")
        );
    }

    #[test]
    fn who_lines_name_the_people_inside_and_boot_names_the_host() {
        use crate::helix::shell::HostId;
        let (st, b, d) = state();
        let inside = who_lines(&st, b);
        assert!(inside.iter().any(|l| l.ends_with("at home")), "{inside:?}");
        let muni = crate::ui::device::Look::named(&st, "municipal", Some(d), 1);
        let boot = boot_lines(&st, HostId::District(d), &muni);
        assert_eq!(boot.len(), 3);
        assert!(
            boot[0].starts_with("HELIX/OS 4.1") && boot[2] == "READY",
            "{boot:?}"
        );
        let dn = st.city.as_ref().unwrap().district(d).name.to_uppercase();
        assert!(boot[0].contains(&dn), "{boot:?}");
        let boot = boot_lines(&st, HostId::Building(b), &muni);
        assert!(boot[0].contains(&building_label(&st, b)), "{boot:?}");
        let street = crate::ui::device::Look::named(&st, "street", Some(d), 1);
        let boot = boot_lines(&st, HostId::District(d), &street);
        assert_eq!(boot[2], "REDY");
        let rig = boot_lines(&st, HostId::Rig, &street);
        assert!(rig.iter().all(|l| !l.contains("HOME RIG")), "{rig:?}");
    }

    #[test]
    fn grades_terminals_and_the_log_read_the_city() {
        let (mut st, b, _) = state();
        assert_eq!(ice_grade(0), "none");
        assert_eq!(ice_grade(160), "white");
        assert_eq!(ice_grade(200), "grey");
        assert_eq!(ice_grade(230), "black");
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
        let lock = nodes_of(&st, b, NodeKind::DoorLock)[0];
        let now = st.net.now(&st);
        st.net.land(
            b,
            HackTarget::Node(lock),
            Target::Node(NodeKind::DoorLock),
            Op::Crack,
            now,
        );
        let log = log_lines(&st, b).join("\n");
        assert!(
            log.contains("door lock") && log.contains("cracked"),
            "{log}"
        );
    }
}
