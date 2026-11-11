//! The world behind the Access Terminal: your standing on a building, the constructs a
//! district terminal sees, and the actions and states the HELIX/OS devices and
//! programs call. Plain functions over the app state that answer in text.

use crate::app::AppState;
use crate::credentials;
use citygen::construct::{BLACK_THRESHOLD, GREY_THRESHOLD, ICE_THRESHOLD};
use citygen::{
    BuildingId, City, DistrictId, LinkKind, NetNode, NodeKind, Place, Pos, SiteNode, StratumKind,
};
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

/// A building's security: its subnet's, but a site's governing security
/// (`sitenet::security`, a live copy's own), so the scan and a backdoor's odds read the
/// copy's.
pub fn security_of(st: &AppState, b: BuildingId) -> u8 {
    // Only a site holds a copy, so a building that is none reads its subnet's here, and
    // asking costs no venue's layout.
    crate::sitenet::security(st, b)
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
    let sec = security_of(st, b);
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

/// What a building-wide command answers on a site below root (Ruling 15).
const ROOT_ONLY: &str = "ROOT ONLY · CRACK THE SERVER";

/// Site `b`'s nodes of `kinds` on the riser's levels, in the Net's order: what HELIX's
/// building-wide commands reach on a site, never across an air gap (Ruling 15).
fn riser_nodes(st: &AppState, b: BuildingId, kinds: &[NodeKind]) -> Vec<(SiteNode, NodeKind)> {
    let Some(net) = crate::sitenet::net_of(st, b) else {
        return Vec::new();
    };
    (net.nodes.iter())
        .filter(|n| kinds.contains(&n.kind) && net.on_riser(n.node.level))
        .map(|n| (n.node, n.kind))
        .collect()
}

/// Whether building `b` is a site, where a building-wide command is root's alone: refused
/// below it (Ruling 15).
fn site_root(st: &AppState, b: BuildingId) -> Result<bool, String> {
    let site = crate::sitenet::is_site(st, b);
    if site && tier_on(st, b) < Tier::Admin {
        return Err(ROOT_ONLY.to_string());
    }
    Ok(site)
}

/// What a building-wide command works on building `b`, and whether `b` is a site: on a
/// site, root's alone, every node of `kinds` on the riser's levels; on any other
/// building, its nodes of `kinds` on the city's Net.
fn reach(
    st: &AppState,
    b: BuildingId,
    kinds: &[NodeKind],
) -> Result<(Vec<(HackTarget, NodeKind)>, bool), String> {
    if site_root(st, b)? {
        let site = (riser_nodes(st, b, kinds).into_iter())
            .map(|(n, k)| (HackTarget::Site(n), k))
            .collect();
        return Ok((site, true));
    }
    let city = (kinds.iter())
        .flat_map(|k| {
            nodes_of(st, b, *k)
                .into_iter()
                .map(|id| (HackTarget::Node(id), *k))
        })
        .collect();
    Ok((city, false))
}

/// Lands `op` on every one of `targets` of building `b` now.
fn land_all(st: &mut AppState, b: BuildingId, targets: &[(HackTarget, NodeKind)], op: Op) {
    let now = st.net.now(st);
    for (t, kind) in targets {
        st.net.land(b, *t, Target::Node(*kind), op, now);
    }
}

/// A command's line for `n` devices: `16 DOOR LOCKS RELEASED · 60 MIN`, one `DOOR LOCK`.
fn counted(n: usize, device: &str, done: &str) -> String {
    let s = if n == 1 { "" } else { "S" };
    format!("{n} {device}{s} {done} · 60 MIN")
}

/// `door release`: a door lock spoofed open for the hour; on a site every door lock and
/// gate switch on the riser, root's alone.
pub fn release_doors(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let (mut locks, site) = reach(st, b, &[NodeKind::DoorLock, NodeKind::GateSwitch])?;
    if !site {
        // The building's first, which opens them all.
        locks.truncate(1);
    }
    if locks.is_empty() {
        return Err("NO DOOR CONTROLLER".to_string());
    }
    if st.net.alert(b, st.net.now(st)) {
        return Err("LOCKDOWN IN FORCE · THE LOCKS HOLD UNTIL THE ALERT ENDS".to_string());
    }
    land_all(st, b, &locks, Op::Spoof);
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s locked doors open for an hour"));
    Ok(if site {
        counted(locks.len(), "DOOR LOCK", "RELEASED")
    } else {
        "DOOR LOCKS RELEASED · 60 MIN".to_string()
    })
}

/// `lift answer`: the lift control spoofed for the hour; on a site every lift on the
/// riser, root's alone.
pub fn answer_lifts(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    let (mut lifts, site) = reach(st, b, &[NodeKind::LiftCtrl])?;
    if !site {
        lifts.truncate(1);
    }
    if lifts.is_empty() {
        return Err("NO LIFT CONTROLLER".to_string());
    }
    land_all(st, b, &lifts, Op::Spoof);
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s lifts answer you"));
    Ok(if site {
        counted(lifts.len(), "LIFT", "ANSWERING")
    } else {
        "LIFTS ANSWERING · 60 MIN".to_string()
    })
}

/// Loops every camera of the building; on a site every camera on the riser, root's
/// alone.
pub fn loop_cams(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    loop_some(st, b, None)
}

/// Loops camera `i` alone: on a site the `i`th on the riser, in the Net's order.
pub fn loop_cam(st: &mut AppState, b: BuildingId, i: usize) -> Result<String, String> {
    loop_some(st, b, Some(i))
}

fn loop_some(st: &mut AppState, b: BuildingId, only: Option<usize>) -> Result<String, String> {
    let (mut cams, _) = reach(st, b, &[NodeKind::Camera])?;
    if let Some(i) = only {
        cams = cams.get(i).copied().into_iter().collect();
    }
    if cams.is_empty() {
        return Err("NO CAMERAS".to_string());
    }
    land_all(st, b, &cams, Op::Blind);
    let name = crate::net::building_name(st, b);
    st.say(format!("`{name}`'s cameras loop for an hour"));
    Ok(counted(cams.len(), "CAMERA", "LOOPED"))
}

/// `alarm off`: root's (`/dev/alarm`), and on a site checked here too.
pub fn alarm_off(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    site_root(st, b)?;
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

/// `/dev/door`: on a site, how many of the riser's door locks and gate switches stand
/// open to a hack on their own node (`locks: 3/16 released`); elsewhere the building's.
pub fn door_state(st: &AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    if st.net.alert(b, now) {
        "lockdown".to_string()
    } else if crate::sitenet::is_site(st, b) {
        let locks = riser_nodes(st, b, &[NodeKind::DoorLock, NodeKind::GateSwitch]);
        let open = (locks.iter())
            .filter(|(n, _)| crate::sitenet::hacked(st, *n, &[Op::Crack, Op::Spoof]))
            .count();
        format!("locks: {open}/{} released", locks.len())
    } else if st.net.unlocked(b, now) {
        match live_until(st, b, NodeKind::DoorLock, &[Op::Crack, Op::Spoof], now) {
            Some(u) => format!("locks: released (until {})", stamp(u)),
            None => "locks: released".to_string(),
        }
    } else {
        "locks: engaged".to_string()
    }
}

/// `/dev/lift`: on a site, how many of the riser's lifts answer you, their stops freed by
/// a spoof on their own node (`lifts: 1/3 answering`); elsewhere the building's.
pub fn lift_state(st: &AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    if crate::sitenet::is_site(st, b) {
        let lifts = riser_nodes(st, b, &[NodeKind::LiftCtrl]);
        let plan = crate::interiors::plan_now(st, b);
        let link_of = |n: &SiteNode| {
            (plan.as_ref()?.links.iter()).position(|k| {
                k.kind == LinkKind::Lift && k.stop(n.level).is_some_and(|s| s.at == n.at)
            })
        };
        let answering = (lifts.iter())
            .filter_map(|(n, _)| link_of(n))
            .filter(|link| crate::sitenet::lift_answers_you(st, b, *link))
            .count();
        return format!("lifts: {answering}/{} answering", lifts.len());
    }
    if !st.net.lifts_answer(b, now) {
        "lifts: stopped".to_string()
    } else if live_until(st, b, NodeKind::LiftCtrl, &[Op::Spoof], now).is_some() {
        "lifts: answering".to_string()
    } else {
        "lifts: normal".to_string()
    }
}

/// `/dev/cam{i}`: camera `i`, on a site the `i`th on the riser in the Net's order.
pub fn cam_state(st: &AppState, b: BuildingId, i: usize) -> String {
    let now = st.net.now(st);
    let node = if crate::sitenet::is_site(st, b) {
        let cams = riser_nodes(st, b, &[NodeKind::Camera]);
        cams.get(i).map(|(n, _)| NetNode::Site(*n))
    } else {
        nodes_of(st, b, NodeKind::Camera)
            .get(i)
            .copied()
            .map(NetNode::City)
    };
    let Some(node) = node else {
        return format!("cam{i}: absent");
    };
    if !st.net.blinded(node, now) {
        return format!("cam{i}: live");
    }
    let until = st
        .net
        .hacks
        .iter()
        .filter(|h| h.target == HackTarget::from(node) && h.op == Op::Blind)
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
    use crate::sitenet::testutil::{crack_server, hack, office_copy};
    use citygen::{BuildingKind, GenParams, Generator, NetNode, NodeKind, SiteNode, SizePreset};
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
                assert!(
                    cams.iter()
                        .all(|c| st.net.blinded(citygen::NetNode::City(*c), now))
                );
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

    /// Site `b`'s nodes of `kinds` on the riser's levels, in the Net's order, by kind.
    fn on_riser(st: &AppState, b: BuildingId, kinds: &[NodeKind]) -> Vec<(SiteNode, NodeKind)> {
        let net = crate::sitenet::net_of(st, b).unwrap();
        (net.nodes.iter())
            .filter(|n| kinds.contains(&n.kind) && net.on_riser(n.node.level))
            .map(|n| (n.node, n.kind))
            .collect()
    }

    #[test]
    fn root_on_a_site_loops_the_risers_cameras_and_none_across_the_gap() {
        let (mut st, b) = office_copy(190);
        let net = crate::sitenet::net_of(&st, b).unwrap();
        let cams: Vec<SiteNode> = (net.nodes.iter())
            .filter(|n| n.kind == NodeKind::Camera)
            .map(|n| n.node)
            .collect();
        assert!(
            cams.iter().any(|c| !net.on_riser(c.level)),
            "a camera across the gap"
        );
        let riser = on_riser(&st, b, &[NodeKind::Camera]).len();
        assert!(riser > 1, "cameras up the riser");
        assert_eq!(
            loop_cams(&mut st, b).unwrap_err(),
            "ROOT ONLY · CRACK THE SERVER"
        );
        crack_server(&mut st, b);
        assert_eq!(
            loop_cams(&mut st, b),
            Ok(format!("{riser} CAMERAS LOOPED · 60 MIN"))
        );
        let now = st.net.now(&st);
        for c in &cams {
            assert_eq!(
                st.net.blinded(NetNode::Site(*c), now),
                net.on_riser(c.level),
                "{c:?}"
            );
        }
        assert!(cam_state(&st, b, 0).starts_with("cam0: looped (until "));
        assert_eq!(cam_state(&st, b, riser), format!("cam{riser}: absent"));
    }

    #[test]
    fn on_a_site_the_devices_count_what_stands_open_on_the_riser() {
        let (mut st, b) = office_copy(190);
        let locks = on_riser(&st, b, &[NodeKind::DoorLock, NodeKind::GateSwitch]);
        let lifts = on_riser(&st, b, &[NodeKind::LiftCtrl]);
        let cams = on_riser(&st, b, &[NodeKind::Camera]);
        let (n, l) = (locks.len(), lifts.len());
        assert!(n > 1 && l > 1 && cams.len() > 1);
        assert_eq!(door_state(&st, b), format!("locks: 0/{n} released"));
        assert_eq!(lift_state(&st, b), format!("lifts: 0/{l} answering"));
        assert_eq!(cam_state(&st, b, 1), "cam1: live");
        // A node cracked or spoofed on its own counts.
        hack(&mut st, locks[0].0, locks[0].1, Op::Crack);
        hack(&mut st, lifts[0].0, NodeKind::LiftCtrl, Op::Spoof);
        assert_eq!(door_state(&st, b), format!("locks: 1/{n} released"));
        assert_eq!(lift_state(&st, b), format!("lifts: 1/{l} answering"));
        // A user's commands are refused; root's work every device on the riser.
        for done in [
            release_doors(&mut st, b),
            answer_lifts(&mut st, b),
            loop_cam(&mut st, b, 1),
            alarm_off(&mut st, b),
        ] {
            assert_eq!(done.unwrap_err(), "ROOT ONLY · CRACK THE SERVER");
        }
        crack_server(&mut st, b);
        assert_eq!(
            release_doors(&mut st, b),
            Ok(format!("{n} DOOR LOCKS RELEASED · 60 MIN"))
        );
        assert_eq!(door_state(&st, b), format!("locks: {n}/{n} released"));
        assert_eq!(
            answer_lifts(&mut st, b),
            Ok(format!("{l} LIFTS ANSWERING · 60 MIN"))
        );
        assert_eq!(lift_state(&st, b), format!("lifts: {l}/{l} answering"));
        assert_eq!(
            loop_cam(&mut st, b, 1).as_deref(),
            Ok("1 CAMERA LOOPED · 60 MIN")
        );
        assert!(cam_state(&st, b, 1).starts_with("cam1: looped (until "));
        assert_eq!(cam_state(&st, b, 0), "cam0: live");
        let now = st.net.now(&st);
        assert!(st.net.blinded(NetNode::Site(cams[1].0), now));
        assert!(!st.net.blinded(NetNode::Site(cams[0].0), now));
        // An alarm: the hacks and the cracked server go; root by a backdoor holds, the
        // locks hold, and a lift answers to a spoof landed since.
        assert_eq!(alarm_off(&mut st, b).unwrap_err(), "NO ALARM");
        crate::net::lock_down(&mut st, b);
        assert_eq!(door_state(&st, b), "lockdown");
        assert_eq!(lift_state(&st, b), format!("lifts: 0/{l} answering"));
        assert_eq!(
            release_doors(&mut st, b).unwrap_err(),
            "ROOT ONLY · CRACK THE SERVER"
        );
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(b, 99, seed, now);
        assert_eq!(
            release_doors(&mut st, b).unwrap_err(),
            "LOCKDOWN IN FORCE · THE LOCKS HOLD UNTIL THE ALERT ENDS"
        );
        assert!(answer_lifts(&mut st, b).is_ok());
        assert_eq!(lift_state(&st, b), format!("lifts: {l}/{l} answering"));
        assert_eq!(alarm_off(&mut st, b).as_deref(), Ok("ALARM CLEARED"));
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
