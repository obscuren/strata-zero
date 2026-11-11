//! Held nodes and their verbs. A crack or a spoof that lands on a camera, an ad board,
//! a lamp grid or a terminal holds it for an hour, from outside a construct or from
//! inside; while you hold it, its kind offers verbs on single keys, and the run's
//! tether stays on it as a dock. Spec
//! `docs/superpowers/specs/2026-11-04-net-nodes-design.md`, "Held nodes and their verbs".

use crate::app::AppState;
use crate::owners::Owner;
use citygen::{BuildingId, DistrictId, NetNode, NodeId, NodeKind, Place, Pos, StratumKind};

/// How far from a held node its verbs reach.
pub const VERB_REACH: i32 = 12;
/// How long a lamp grid stays dark.
pub const LIGHTS_OUT_MINUTES: u64 = 20;
/// Beats a decoy holds the ICE.
pub const DECOY_BEATS: u8 = 4;
/// Walkers this near a hijacked ad stare at it.
pub const STARE_REACH: i32 = 6;
/// Walkers this near a hijacked ad go to it when the lure goes up.
pub const LURE_REACH: i32 = 12;
/// You on the street this near a hijacked ad see its line.
pub const LINE_REACH: i32 = 12;
/// A held node is drawn in the green of a landed run: its glyph and its glow.
pub const HELD_FG: ratatui::style::Color = ratatui::style::Color::Rgb(80, 230, 120);
pub const HELD_BG: ratatui::style::Color = ratatui::style::Color::Rgb(15, 58, 31);
/// The floor of a room a decoy floods.
pub const FLOOD_BG: ratatui::style::Color = ratatui::style::Color::Rgb(60, 0, 46);

/// What a held node can be made to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Verb {
    Tap,
    Loop,
    Hijack,
    Decoy,
    LightsOut,
    Shell,
}

impl Verb {
    pub fn key(self) -> char {
        match self {
            Verb::Tap => 'f',
            Verb::Loop | Verb::LightsOut => 'o',
            Verb::Hijack => 'i',
            Verb::Decoy => 'z',
            Verb::Shell => 'r',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Verb::Tap => "tap",
            Verb::Loop => "loop",
            Verb::Hijack => "hijack",
            Verb::Decoy => "decoy",
            Verb::LightsOut => "lights out",
            Verb::Shell => "shell",
        }
    }

    /// A verb you may use again while the hold lasts (the feed toggles, the shell
    /// opens again); the others are spent once.
    fn again(self) -> bool {
        matches!(self, Verb::Tap | Verb::Shell)
    }
}

/// Every key a verb answers on.
pub const VERB_KEYS: [char; 5] = ['f', 'o', 'i', 'z', 'r'];

/// The verbs a kind of node offers; none for a kind you can't hold.
pub fn verbs(kind: NodeKind) -> &'static [Verb] {
    match kind {
        NodeKind::Camera => &[Verb::Tap, Verb::Loop],
        NodeKind::AdBoard => &[Verb::Hijack, Verb::Decoy],
        NodeKind::LampGrid => &[Verb::LightsOut],
        NodeKind::Terminal => &[Verb::Shell],
        _ => &[],
    }
}

/// Whether a landed crack or spoof holds this kind of node.
pub fn holdable(kind: NodeKind) -> bool {
    !verbs(kind).is_empty()
}

/// A node you hold, until a game minute, and the verbs spent on it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hold {
    pub node: NetNode,
    pub owner: Owner,
    pub until: u64,
    pub used: Vec<Verb>,
}

/// What a hijacked ad says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Line {
    /// The building named for what it does to you.
    Smear,
    /// A fake offer: walkers near come to look.
    Lure,
    /// Your handle on their glass.
    Tag,
}

impl Line {
    pub fn key(self) -> char {
        match self {
            Line::Smear => 's',
            Line::Lure => 'l',
            Line::Tag => 't',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Line::Smear => "Smear",
            Line::Lure => "Lure",
            Line::Tag => "Tag",
        }
    }
}

/// A building's holo ads saying your line, until the hold on the board runs out.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hijack {
    pub node: NodeId,
    pub building: BuildingId,
    pub line: Line,
    pub text: String,
    pub until: u64,
}

/// An ad board flooding its room: the construct's asleep ICE goes to look.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Decoy {
    pub building: BuildingId,
    pub node: NodeId,
    /// The board's tile in the construct.
    pub to: Pos,
    pub beats: u8,
}

/// A landed crack or spoof: the node is yours for an hour (a fresh hold on a node you
/// already held starts the hour again and keeps what was spent). The log line.
pub fn take(st: &mut AppState, node: NetNode, owner: Owner, now: u64) -> String {
    let until = now + game::HACK_MINUTES;
    if let Some(id) = node.city() {
        for j in st.net.hijacks.iter_mut().filter(|j| j.node == id) {
            j.until = until;
        }
    }
    match st.net.holds.iter_mut().find(|h| h.node == node) {
        Some(h) => h.until = until,
        None => st.net.holds.push(Hold {
            node,
            owner,
            until,
            used: Vec::new(),
        }),
    }
    let kind = kind_of(st, node).map_or("node", NodeKind::label);
    let keys: Vec<String> = verbs_of(st, node)
        .iter()
        .map(|v| format!("[{}] {}", v.key(), v.label()))
        .collect();
    if keys.is_empty() {
        format!("{kind} held · an hour")
    } else {
        format!("{kind} held · an hour · {}", keys.join(" "))
    }
}

/// What kind a node is: a city node's from the city's Net; a site node's from its
/// building's construct (its devices come with the site's Net).
pub fn kind_of(st: &AppState, node: NetNode) -> Option<NodeKind> {
    match node {
        NetNode::City(id) => st
            .city
            .as_ref()
            .and_then(|c| c.net.nodes.get(id.index()))
            .map(|n| n.kind),
        NetNode::Site(n) => st.constructs.get(n.building)?.device(n).map(|d| d.kind),
    }
}

/// The hold on a node, while it lasts.
pub fn held(st: &AppState, node: NetNode, now: u64) -> Option<&Hold> {
    st.net
        .holds
        .iter()
        .find(|h| h.node == node && now < h.until)
}

/// You hold one of the owner's terminals: for the hold you log in on its host as a
/// user (root stays the server's).
pub fn terminal_held(st: &AppState, owner: Owner) -> bool {
    let now = st.net.now(st);
    st.net.holds.iter().any(|h| {
        h.owner == owner && now < h.until && kind_of(st, h.node) == Some(NodeKind::Terminal)
    })
}

/// A camera you hold has been looped.
pub fn looped(st: &AppState, node: NetNode, now: u64) -> bool {
    held(st, node, now).is_some_and(|h| h.used.contains(&Verb::Loop))
}

/// The clock moves: holds run out, and with them a feed and a hijack; a district's
/// lights come back on; a feed closes once you are off the Net, and a site camera's open
/// feed keeps its floor drawn; a decoy ends when you leave its construct. The cities and
/// towns held as you left them keep the same clock: their holds, hijacks and lights out
/// run out as the live city's do, so they look from outside as they will when you walk
/// in again.
pub fn tick(st: &mut AppState) {
    let now = st.net.now(st);
    expire(&mut st.net, now);
    let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
    if let Some(f) = st.net.feed
        && (!jacked || held(st, f, now).is_none())
    {
        st.net.feed = None;
    }
    // The feed paints its floor each frame: drawn once and kept, not drawn afresh.
    if let (Some(NetNode::Site(n)), Some(city)) = (st.net.feed, st.city.as_ref()) {
        st.interiors.ensure(city, n.building, n.level);
    }
    let here = st.player.as_ref().map(|p| p.place);
    if st
        .net
        .decoy
        .as_ref()
        .is_some_and(|d| here.and_then(Place::construct) != Some(d.building))
    {
        st.net.decoy = None;
    }
    say_lines(st);
    relight(st);
    for stay in crate::outside::held_stays_mut(st) {
        expire(&mut stay.net, now);
        relight_city(&mut stay.city, &stay.net);
    }
}

/// What of a city's Net state has run out by game minute `now` gone: holds, the hijacks
/// they carried, lights out.
fn expire(net: &mut crate::net::NetState, now: u64) {
    net.holds.retain(|h| now < h.until);
    let holds = &net.holds;
    net.hijacks
        .retain(|j| now < j.until && holds.iter().any(|h| h.node == NetNode::City(j.node)));
    net.lights_out.retain(|_, until| now < *until);
}

/// The street's lamps cut in the districts `NetState::lights_out` holds, and burning
/// everywhere else. Each tick, and as a saved run is reopened
/// (`runs::snapshot::after_restore`): the street's `dark` is never saved.
pub fn relight(st: &mut AppState) {
    if let Some(city) = st.city.as_mut() {
        relight_city(city, &st.net);
    }
}

/// `city`'s street lamps cut in the districts `net.lights_out` holds, burning elsewhere.
fn relight_city(city: &mut citygen::City, net: &crate::net::NetState) {
    let dark: std::collections::BTreeSet<DistrictId> = net.lights_out.keys().copied().collect();
    if let Some(s) = city.stratum_mut(StratumKind::Street)
        && s.dark != dark
    {
        s.dark = dark;
    }
}

/// Where a node is drawn on a place: on the open Net its tile there; inside a
/// construct its tile in it; a site's node on its own level of the site's Net, at its
/// device's tile.
pub fn node_pos(st: &AppState, place: Place, node: NetNode) -> Option<Pos> {
    let id = match node {
        NetNode::City(id) => id,
        NetNode::Site(n) => {
            return match place {
                Place::SiteNet { building, level }
                    if (building, level) == (n.building, n.level) =>
                {
                    st.constructs.site(building)?.device(n).map(|d| d.at)
                }
                _ => None,
            };
        }
    };
    match place {
        Place::Stratum(StratumKind::Net) => {
            Some(st.city.as_ref()?.net.nodes.get(id.index())?.cyber)
        }
        Place::Construct(b) | Place::SiteNet { building: b, .. } => st
            .constructs
            .get(b)?
            .nodes
            .iter()
            .find(|(n, _)| *n == id)
            .map(|(_, p)| *p),
        _ => None,
    }
}

/// Every node you hold drawn on a place, nearest you first, with its tile.
pub fn held_on(st: &AppState, place: Place) -> Vec<(NetNode, Pos)> {
    let now = st.net.now(st);
    let me = st.player.as_ref().map_or(Pos::new(0, 0), |p| p.pos);
    let mut out: Vec<(NetNode, Pos)> = st
        .net
        .holds
        .iter()
        .filter(|h| now < h.until)
        .filter_map(|h| Some((h.node, node_pos(st, place, h.node)?)))
        .collect();
    out.sort_by_key(|(_, at)| at.chebyshev(me));
    out
}

/// The held node nearest you on your map, within `VERB_REACH`, and its tile.
pub fn nearest(st: &AppState) -> Option<(NetNode, Pos)> {
    let p = st.player.as_ref()?;
    held_on(st, p.place)
        .into_iter()
        .next()
        .filter(|(_, at)| at.chebyshev(p.pos) <= VERB_REACH)
}

/// The verbs a held node offers where you stand now: spent ones gone (but the feed and
/// the shell), and a decoy only inside its own construct with ICE standing.
pub fn verbs_of(st: &AppState, node: NetNode) -> Vec<Verb> {
    let Some(kind) = kind_of(st, node) else {
        return Vec::new();
    };
    let building = match node {
        NetNode::City(id) => st
            .city
            .as_ref()
            .and_then(|c| c.net.nodes.get(id.index()))
            .and_then(|n| n.building),
        NetNode::Site(n) => Some(n.building),
    };
    let now = st.net.now(st);
    let used = held(st, node, now)
        .map(|h| h.used.clone())
        .unwrap_or_default();
    verbs(kind)
        .iter()
        .copied()
        .filter(|v| v.again() || !used.contains(v))
        .filter(|v| *v != Verb::Decoy || decoy_ok(st, building))
        .collect()
}

/// A decoy works inside the ad board's own construct while ICE stands in it.
fn decoy_ok(st: &AppState, building: Option<BuildingId>) -> bool {
    let (Some(b), Some(p)) = (building, st.player.as_ref()) else {
        return false;
    };
    p.place.construct() == Some(b)
        && st
            .constructs
            .get(b)
            .is_some_and(|k| k.guards.iter().any(|g| g.standing && !g.inert))
}

/// What a verb did, for the screen: a line to log, a shell to open on a host (keyed
/// by the node's tile), or the hijack's lines to pick from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerbDone {
    Said(String),
    Shell(crate::helix::shell::HostId, (Place, Pos)),
    Pick(NodeId),
}

/// A verb key: the verb of the held node nearest you that answers it. Calm only.
pub fn use_verb(st: &mut AppState, key: char) -> Result<VerbDone, String> {
    if st.net.engaged.is_some() {
        return Err("not in a fight".to_string());
    }
    let (node, at) = nearest(st).ok_or("nothing held in reach")?;
    let kind = kind_of(st, node).ok_or("nothing held in reach")?;
    let Some(verb) = verbs(kind).iter().copied().find(|v| v.key() == key) else {
        return Err(format!("the {} has nothing on [{key}]", kind.label()));
    };
    if !verbs_of(st, node).contains(&verb) {
        return Err(match verb {
            Verb::Decoy => "a decoy works inside its construct, with ICE in it".to_string(),
            _ => format!("{} spent on this hold", verb.label()),
        });
    }
    let now = st.net.now(st);
    let done = match verb {
        Verb::Tap => {
            if st.net.feed == Some(node) {
                st.net.feed = None;
                VerbDone::Said("feed closed".to_string())
            } else {
                st.net.feed = Some(node);
                VerbDone::Said(format!("tap · {}", feed_title(st, node)))
            }
        }
        Verb::Loop => {
            spend(st, node, verb);
            let left = held(st, node, now).map_or(0, |h| h.until - now);
            VerbDone::Said(format!("loop · the camera sees nothing for {left} min"))
        }
        Verb::Hijack => VerbDone::Pick(node.city().ok_or("the board answers nothing")?),
        Verb::Decoy => VerbDone::Said(decoy(st, node.city().ok_or("no decoy there")?, at)?),
        Verb::LightsOut => VerbDone::Said(lights_out(
            st,
            node.city().ok_or("the grid answers nothing")?,
            now,
        )?),
        Verb::Shell => {
            let host = host_of(st, node).ok_or("no shell answers there")?;
            let place = st
                .player
                .as_ref()
                .map_or(Place::Stratum(StratumKind::Net), |p| p.place);
            VerbDone::Shell(host, (place, at))
        }
    };
    Ok(done)
}

/// Marks a verb spent on a hold.
pub fn spend(st: &mut AppState, node: NetNode, verb: Verb) {
    if let Some(h) = st.net.holds.iter_mut().find(|h| h.node == node)
        && !h.used.contains(&verb)
    {
        h.used.push(verb);
    }
}

/// What a node's building is called, or `the street` for a district's.
fn place_name(st: &AppState, node: NetNode) -> String {
    match node {
        NetNode::City(id) => st
            .city
            .as_ref()
            .and_then(|c| c.net.nodes.get(id.index()))
            .and_then(|n| n.building),
        NetNode::Site(n) => Some(n.building),
    }
    .map(|b| crate::net::building_name(st, b))
    .unwrap_or_else(|| "the street".to_string())
}

/// `CAM · KIROFLUX MART`: what a camera's feed is headed.
pub fn feed_title(st: &AppState, node: NetNode) -> String {
    format!("CAM · {}", place_name(st, node).to_uppercase())
}

/// The host a terminal node answers for: its building's, or its district's for a
/// street terminal.
fn host_of(st: &AppState, node: NetNode) -> Option<crate::helix::shell::HostId> {
    use crate::helix::shell::HostId;
    let id = match node {
        NetNode::Site(n) => return Some(HostId::Building(n.building)),
        NetNode::City(id) => id,
    };
    let city = st.city.as_ref()?;
    Some(match crate::owners::owner_of(city, id)? {
        Owner::Building(b) => HostId::Building(b),
        Owner::District(d) => HostId::District(d),
    })
}

/// The lamp grid's district goes dark for `LIGHTS_OUT_MINUTES`; by day there is
/// nothing to cut.
fn lights_out(st: &mut AppState, node: NodeId, now: u64) -> Result<String, String> {
    if !citygen::lighting::lamps_on(st.minute_of_day()) {
        return Err("the lamps are off by day".to_string());
    }
    let city = st.city.as_ref().ok_or("no city")?;
    let Some(Owner::District(d)) = crate::owners::owner_of(city, node) else {
        return Err("the grid answers nothing".to_string());
    };
    let name = city
        .district_opt(d)
        .map_or_else(|| city.name.clone(), |x| x.name.clone());
    spend(st, NetNode::City(node), Verb::LightsOut);
    st.net.lights_out.insert(d, now + LIGHTS_OUT_MINUTES);
    tick(st);
    Ok(format!(
        "lights out · {name}'s street lamps die · {LIGHTS_OUT_MINUTES} min"
    ))
}

/// The ad board floods its room: every standing piece asleep goes to look.
fn decoy(st: &mut AppState, node: NodeId, at: Pos) -> Result<String, String> {
    let Some(b) = st.player.as_ref().and_then(|p| p.place.construct()) else {
        return Err("a decoy works inside its construct".to_string());
    };
    spend(st, NetNode::City(node), Verb::Decoy);
    st.net.decoy = Some(Decoy {
        building: b,
        node,
        to: at,
        beats: DECOY_BEATS,
    });
    Ok(format!(
        "decoy · the board floods its room · {DECOY_BEATS} beats"
    ))
}

/// One beat of a construct passed: the decoy wears.
pub fn decoy_beat(st: &mut AppState) {
    if let Some(d) = st.net.decoy.as_mut() {
        d.beats = d.beats.saturating_sub(1);
        if d.beats == 0 {
            st.net.decoy = None;
            st.say("the decoy's flood dies down");
        }
    }
}

/// Where a decoy draws a construct's asleep pieces, while it lasts.
pub fn decoy_draw(st: &AppState, b: BuildingId) -> Option<Pos> {
    st.net
        .decoy
        .as_ref()
        .filter(|d| d.building == b && d.beats > 0)
        .map(|d| d.to)
}

/// The floor of the room a decoy floods, while it lasts.
pub fn decoy_room(st: &AppState, b: BuildingId) -> Vec<Pos> {
    let (Some(to), Some(k)) = (decoy_draw(st, b), st.constructs.get(b)) else {
        return Vec::new();
    };
    let Some(room) = k
        .rooms
        .iter()
        .find(|r| r.contains(to))
        .or_else(|| k.rooms.iter().min_by_key(|r| r.center().chebyshev(to)))
    else {
        return Vec::new();
    };
    room.iter()
        .filter(|p| k.tiles.get(*p).is_some_and(|t| t.is_walkable()))
        .collect()
}

/// The three lines a hijack offers for a building's ads.
pub fn hijack_lines(st: &AppState, b: BuildingId) -> [(Line, String); 3] {
    let name = crate::net::building_name(st, b).to_uppercase();
    let handle = st.character.handle.to_uppercase();
    let handle = if handle.is_empty() {
        "SOMEONE".to_string()
    } else {
        handle
    };
    [
        (Line::Smear, format!("{name} SELLS YOUR FACE")),
        (Line::Lure, format!("FREE NOODLES · {name} · NOW")),
        (Line::Tag, format!("{handle} WAS HERE")),
    ]
}

/// The street tiles of a building's holo ads: its ad board nodes' anchors.
pub fn ad_tiles(st: &AppState, b: BuildingId) -> Vec<Pos> {
    st.city
        .as_ref()
        .map_or_else(Vec::new, |c| ad_tiles_in(c, b))
}

/// The street tiles of building `b`'s holo ads in `city`.
fn ad_tiles_in(city: &citygen::City, b: BuildingId) -> Vec<Pos> {
    city.net
        .nodes
        .iter()
        .filter(|n| n.building == Some(b) && n.kind == NodeKind::AdBoard)
        .filter(|n| n.anchor.0 == StratumKind::Street)
        .map(|n| n.anchor.1)
        .collect()
}

/// Every hijacked ad on the street, with what it says, while its hold lasts.
pub fn hijacked_ads(st: &AppState) -> Vec<(Pos, &Hijack)> {
    match st.city.as_ref() {
        Some(city) => hijacked_ads_in(city, &st.net, st.net.now(st)),
        None => Vec::new(),
    }
}

/// Every hijacked ad on `city`'s street at game minute `now` (its Net state `net`),
/// with what it says: the live city's, or one held as you left it.
pub fn hijacked_ads_in<'a>(
    city: &citygen::City,
    net: &'a crate::net::NetState,
    now: u64,
) -> Vec<(Pos, &'a Hijack)> {
    net.hijacks
        .iter()
        .filter(|j| now < j.until)
        .flat_map(|j| {
            ad_tiles_in(city, j.building)
                .into_iter()
                .map(move |p| (p, j))
        })
        .collect()
}

/// On foot on the street near a hijacked ad, it says its line over your map, under
/// your handle; again a while after it goes down.
fn say_lines(st: &mut AppState) {
    use crate::missions::asides::{self, Speaker};
    let Some(me) = st
        .player
        .as_ref()
        .filter(|p| p.place == Place::Stratum(StratumKind::Street))
        .map(|p| p.pos)
    else {
        return;
    };
    if st.net.hijacks.is_empty() || st.clock_tick() < st.net.line_next || !asides::live(st) {
        return;
    }
    let Some((at, text)) = hijacked_ads(st)
        .into_iter()
        .filter(|(at, _)| at.chebyshev(me) <= LINE_REACH)
        .min_by_key(|(at, _)| at.chebyshev(me))
        .map(|(at, j)| (at, j.text.clone()))
    else {
        return;
    };
    let speaker = Speaker::Tile(Place::Stratum(StratumKind::Street), at);
    if st.missions.asides.iter().any(|a| a.speaker == speaker) {
        return;
    }
    let handle = st.character.handle.clone();
    let line = format!("▸ {text}");
    st.net.line_next = st.clock_tick() + asides::lasts(&line) + asides::READ_GAP * 4;
    asides::say(st, speaker, handle, crate::ui::chrome::MAGENTA, line);
}

/// Whether someone at `p` on the street is staring at a hijacked ad.
pub fn staring(st: &AppState, p: Pos) -> bool {
    !st.net.hijacks.is_empty()
        && hijacked_ads(st)
            .iter()
            .any(|(at, _)| at.chebyshev(p) <= STARE_REACH)
}

/// The pavement in front of an ad: the first walkable street tile beside it outside
/// its building.
fn in_front(st: &AppState, at: Pos) -> Option<Pos> {
    let city = st.city.as_ref()?;
    citygen::Dir::ALL.iter().map(|d| at.step(*d)).find(|q| {
        city.tile(StratumKind::Street, *q)
            .is_some_and(|t| t.is_walkable() && t.building.is_none())
    })
}

/// The hijack goes up on the board's building: its ads say the line until the hold
/// ends; the lure sends the walkers near to the nearest ad. The log line.
pub fn hijack(st: &mut AppState, node: NodeId, line: Line) -> String {
    let now = st.net.now(st);
    let Some(hold) = held(st, NetNode::City(node), now).cloned() else {
        return "the hold is gone".to_string();
    };
    let Some(b) = st
        .city
        .as_ref()
        .and_then(|c| c.net.nodes.get(node.index()))
        .and_then(|n| n.building)
    else {
        return "the board answers nothing".to_string();
    };
    let text = hijack_lines(st, b)
        .into_iter()
        .find(|(l, _)| *l == line)
        .map(|(_, t)| t)
        .unwrap_or_default();
    spend(st, NetNode::City(node), Verb::Hijack);
    st.net.hijacks.retain(|j| j.building != b);
    st.net.hijacks.push(Hijack {
        node,
        building: b,
        line,
        text: text.clone(),
        until: hold.until,
    });
    let sent = if line == Line::Lure { lure(st, b) } else { 0 };
    let name = crate::net::building_name(st, b);
    match sent {
        0 => format!("hijack · `{name}`'s ads say {text}"),
        n => format!(
            "hijack · `{name}`'s ads say {text} · {n} walker{} come to look",
            if n == 1 { "" } else { "s" }
        ),
    }
}

/// The lure: walkers on the street near the building's ads head for the pavement in
/// front of the nearest one. How many went.
fn lure(st: &mut AppState, b: BuildingId) -> usize {
    let fronts: Vec<(Pos, Pos)> = ad_tiles(st, b)
        .into_iter()
        .filter_map(|a| Some((a, in_front(st, a)?)))
        .collect();
    let Some(world) = st.world.as_mut() else {
        return 0;
    };
    let mut asks = Vec::new();
    for person in world.people() {
        if world.is_held(person.id) || world.spot_of(person.id).is_some() {
            continue;
        }
        let Some(at) = world.pos_of(person.id) else {
            continue;
        };
        let Some((_, front)) = fronts
            .iter()
            .filter(|(ad, _)| ad.chebyshev(at) <= LURE_REACH)
            .min_by_key(|(ad, _)| ad.chebyshev(at))
        else {
            continue;
        };
        asks.push((person.id, *front, person.goal));
    }
    let n = asks.len();
    for (who, front, goal) in asks {
        world.request(who, sim::Dest::Spot(front), goal);
    }
    n
}

/// The dock on the held node nearest you, once you are within its verbs' reach: the
/// run's tether that stayed, its label the node and the time left, its second row the
/// verbs and their keys. Further off a held node is only its green.
pub fn dock_view(st: &AppState, place: Place) -> Option<crate::render::tether::TetherView> {
    use crate::render::tether::{TetherState, TetherView};
    let p = st.player.as_ref().filter(|p| p.place == place)?;
    if st.net.engaged.is_some() {
        return None;
    }
    let (node, at) = nearest(st)?;
    let now = st.net.now(st);
    let hold = held(st, node, now)?;
    let kind = kind_of(st, node)?;
    let open = verbs_of(st, node);
    let mut name = open
        .iter()
        .map(|v| format!("[{}] {}", v.key(), v.label()))
        .collect::<Vec<_>>()
        .join("  ");
    let mut spent = |what: &str| {
        if !name.is_empty() {
            name.push_str(" · ");
        }
        name.push_str(what);
    };
    for (verb, what) in [
        (Verb::Loop, "looped"),
        (Verb::Hijack, "hijacked"),
        (Verb::LightsOut, "lights out"),
        (Verb::Decoy, "decoyed"),
    ] {
        if hold.used.contains(&verb) {
            spent(what);
        }
    }
    let label = kind.label().to_uppercase();
    Some(TetherView {
        target: at,
        me: p.pos,
        keep: Vec::new(),
        on_self: false,
        op: label.clone(),
        done: format!("{label}  HELD {}m", (hold.until - now).max(1)),
        odds: 100,
        name,
        turns: 1,
        work: 1,
        phase: 0.0,
        state: TetherState::Landed { age: 0 },
    })
}

/// Every held node on the map in the landed run's green: glyph and glow.
pub struct HeldOverlay<'a> {
    pub tiles: &'a [Pos],
}

impl crate::render::viewport::Overlay for HeldOverlay<'_> {
    fn draw(
        &self,
        _map: &crate::render::map::MapRef,
        origin: Pos,
        area: ratatui::layout::Rect,
        _tick: u64,
        buf: &mut ratatui::buffer::Buffer,
        _style: &crate::render::style::StylePack,
    ) {
        for p in self.tiles {
            if let Some((sx, sy)) = crate::render::viewport::tile_to_screen(area, origin, *p) {
                let cell = &mut buf[(sx, sy)];
                cell.set_fg(HELD_FG).set_bg(HELD_BG);
                cell.modifier.insert(ratatui::style::Modifier::BOLD);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::{crack_to_landing, node_cyber, under_kiroflux};
    use game::{HackTarget, ItemId};

    /// Kiroflux Mart's camera held from the lane under it.
    fn camera_held() -> (AppState, NodeId) {
        let (mut st, b) = under_kiroflux();
        let (cam, at) = node_cyber(&st, b, NodeKind::Camera);
        st.player.as_mut().unwrap().pos = Pos::new(at.x, at.y + 1);
        crack_to_landing(&mut st, HackTarget::Node(cam));
        (st, cam)
    }

    #[test]
    fn an_old_hold_reads_as_a_city_node() {
        #[derive(serde::Serialize)]
        struct OldHold {
            node: NodeId,
            owner: Owner,
            until: u64,
            used: Vec<Verb>,
        }
        let old = OldHold {
            node: NodeId(9),
            owner: Owner::Building(BuildingId(2)),
            until: 70,
            used: vec![Verb::Loop],
        };
        let back: Hold = rmp_serde::from_slice(&rmp_serde::to_vec_named(&old).unwrap()).unwrap();
        assert_eq!(back.node, citygen::NetNode::City(NodeId(9)));
        assert_eq!(back.used, vec![Verb::Loop]);
    }

    #[test]
    fn a_site_hold_and_its_feed_come_back() {
        let n = citygen::NetNode::Site(citygen::SiteNode {
            building: BuildingId(4),
            level: 2,
            at: Pos::new(6, 3),
        });
        let mut net = crate::net::NetState::default();
        net.holds.push(Hold {
            node: n,
            owner: Owner::Building(BuildingId(4)),
            until: 80,
            used: vec![Verb::Tap],
        });
        net.feed = Some(n);
        let back: crate::net::NetState =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&net).unwrap()).unwrap();
        assert_eq!((back.holds[0].node, back.feed), (n, Some(n)));
    }

    #[test]
    fn a_crack_holds_a_camera_for_an_hour() {
        let (st, cam) = camera_held();
        let now = st.net.now(&st);
        let h = held(&st, citygen::NetNode::City(cam), now).expect("held");
        assert_eq!(h.until, now + game::HACK_MINUTES);
        assert!(
            st.log
                .iter()
                .any(|e| e.text.contains("camera held · an hour · [f] tap [o] loop"))
        );
    }

    #[test]
    fn spoof_holds_quietly() {
        let (mut st, b) = under_kiroflux();
        let (ad, at) = node_cyber(&st, b, NodeKind::AdBoard);
        st.player.as_mut().unwrap().pos = Pos::new(at.x, at.y - 1);
        if !st.character.loaded.contains(&ItemId::new("spoof")) {
            st.character.loaded.push(ItemId::new("spoof"));
        }
        st.net.force_roll = Some(10);
        crate::net::start_run(&mut st, ItemId::new("spoof"), HackTarget::Node(ad)).unwrap();
        while st.net.run.is_some() {
            crate::net::step_run(&mut st);
        }
        let now = st.net.now(&st);
        assert!(held(&st, citygen::NetNode::City(ad), now).is_some());
        let spoof = st.net.traces.get(&b).map_or(0, |t| t.heat);
        let (mut st2, _) = under_kiroflux();
        st2.player.as_mut().unwrap().pos = Pos::new(at.x, at.y - 1);
        crack_to_landing(&mut st2, HackTarget::Node(ad));
        let crack = st2.net.traces.get(&b).map_or(0, |t| t.heat);
        assert!(spoof < crack, "spoof {spoof} is quieter than crack {crack}");
    }

    #[test]
    fn a_verb_key_acts_on_the_nearest_hold() {
        let (mut st, cam) = camera_held();
        let now = st.net.now(&st);
        assert!(!st.net.blinded(citygen::NetNode::City(cam), now));
        let done = use_verb(&mut st, 'o').unwrap();
        assert!(
            matches!(done, VerbDone::Said(ref l) if l.starts_with("loop")),
            "{done:?}"
        );
        assert!(
            st.net.blinded(citygen::NetNode::City(cam), now),
            "the loop blinds it"
        );
        // Spent: the loop is gone from its verbs, the tap still there.
        assert_eq!(verbs_of(&st, citygen::NetNode::City(cam)), vec![Verb::Tap]);
        assert!(use_verb(&mut st, 'o').is_err());
        // Tap toggles the feed.
        use_verb(&mut st, 'f').unwrap();
        assert_eq!(st.net.feed, Some(citygen::NetNode::City(cam)));
        use_verb(&mut st, 'f').unwrap();
        assert_eq!(st.net.feed, None);
        // A key the camera has no verb on.
        assert!(
            use_verb(&mut st, 'r')
                .unwrap_err()
                .contains("nothing on [r]")
        );
    }

    #[test]
    fn a_verb_out_of_reach_says_so() {
        let (mut st, _) = camera_held();
        st.player.as_mut().unwrap().pos.x -= VERB_REACH + 2;
        assert_eq!(
            use_verb(&mut st, 'f'),
            Err("nothing held in reach".to_string())
        );
        assert!(st.net.feed.is_none());
    }

    #[test]
    fn a_hold_runs_out() {
        let (mut st, cam) = camera_held();
        st.net.feed = Some(citygen::NetNode::City(cam));
        st.net.holds.iter_mut().for_each(|h| h.until = 0);
        tick(&mut st);
        assert!(st.net.holds.is_empty());
        assert_eq!(st.net.feed, None, "the feed goes with it");
        assert!(dock_view(&st, Place::Stratum(StratumKind::Net)).is_none());
    }

    #[test]
    fn the_dock_shows_only_within_reach() {
        let (mut st, _) = camera_held();
        assert!(dock_view(&st, Place::Stratum(StratumKind::Net)).is_some());
        st.player.as_mut().unwrap().pos.x -= VERB_REACH + 2;
        assert!(
            dock_view(&st, Place::Stratum(StratumKind::Net)).is_none(),
            "far off, the node is only green"
        );
        assert_eq!(held_on(&st, Place::Stratum(StratumKind::Net)).len(), 1);
    }

    #[test]
    fn the_dock_reads_the_verbs() {
        let (mut st, _) = camera_held();
        let v = dock_view(&st, Place::Stratum(StratumKind::Net)).expect("a dock");
        assert_eq!(v.done, "CAMERA  HELD 60m");
        assert_eq!(v.name, "[f] tap  [o] loop");
        use_verb(&mut st, 'o').unwrap();
        let v = dock_view(&st, Place::Stratum(StratumKind::Net)).unwrap();
        assert_eq!(v.name, "[f] tap · looped");
    }

    #[test]
    fn holds_outlast_an_alarm() {
        let (mut st, cam) = camera_held();
        let b = st.city.as_ref().unwrap().net.nodes[cam.index()]
            .building
            .unwrap();
        use_verb(&mut st, 'o').unwrap();
        let now = st.net.now(&st);
        st.net.alarm(b, now);
        assert!(
            st.net.blinded(citygen::NetNode::City(cam), now),
            "the loop is the hold's, not a hack"
        );
    }

    #[test]
    fn a_held_terminal_logs_you_in_as_a_user() {
        use crate::helix::shell::HostId;
        let (mut st, b) = under_kiroflux();
        let city = st.city.as_ref().unwrap();
        let term = city
            .net
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Terminal && n.building == Some(b))
            .map(|n| n.id);
        let d = city.buildings[b.index()].district;
        let street = city
            .net
            .nodes
            .iter()
            .find(|n| {
                n.kind == NodeKind::Terminal
                    && crate::owners::owner_of(city, n.id) == Some(Owner::District(d))
            })
            .map(|n| n.id)
            .expect("a street terminal");
        let now = st.net.now(&st);
        assert_eq!(
            crate::helix::hosts::user_for(&st, HostId::District(d)),
            "guest"
        );
        take(
            &mut st,
            citygen::NetNode::City(street),
            Owner::District(d),
            now,
        );
        assert_eq!(
            crate::helix::hosts::user_for(&st, HostId::District(d)),
            "user"
        );
        let term = term.expect("Kiroflux Mart has a terminal");
        assert_eq!(crate::access::tier_on(&st, b), crate::access::Tier::Guest);
        take(
            &mut st,
            citygen::NetNode::City(term),
            Owner::Building(b),
            now,
        );
        assert_eq!(crate::access::tier_on(&st, b), crate::access::Tier::User);
        assert_eq!(
            crate::helix::hosts::user_for(&st, HostId::Building(b)),
            "user"
        );
        // The hour over, you're a guest again.
        st.net.holds.iter_mut().for_each(|h| h.until = 0);
        assert_eq!(
            crate::helix::hosts::user_for(&st, HostId::District(d)),
            "guest"
        );
        assert_eq!(crate::access::tier_on(&st, b), crate::access::Tier::Guest);
    }

    #[test]
    fn hijack_offers_three_lines() {
        let (mut st, b) = under_kiroflux();
        st.character.handle = "Vex".to_string();
        let lines = hijack_lines(&st, b);
        assert_eq!(lines[0].1, "KIROFLUX MART SELLS YOUR FACE");
        assert_eq!(lines[1].1, "FREE NOODLES · KIROFLUX MART · NOW");
        assert_eq!(lines[2].1, "VEX WAS HERE");
        let (ad, at) = node_cyber(&st, b, NodeKind::AdBoard);
        st.player.as_mut().unwrap().pos = Pos::new(at.x, at.y - 1);
        crack_to_landing(&mut st, HackTarget::Node(ad));
        assert_eq!(use_verb(&mut st, 'i'), Ok(VerbDone::Pick(ad)));
        let line = hijack(&mut st, ad, Line::Tag);
        assert!(line.contains("VEX WAS HERE"), "{line}");
        let ads = hijacked_ads(&st);
        assert!(!ads.is_empty());
        assert!(ads.iter().all(|(_, j)| j.text == "VEX WAS HERE"));
        // Spent once; the decoy only inside.
        assert!(use_verb(&mut st, 'i').is_err());
        assert!(use_verb(&mut st, 'z').unwrap_err().contains("inside"));
        // On foot near the ads, they say the line under your handle.
        let (ad_at, _) = hijacked_ads(&st)[0];
        let p = st.player.as_mut().unwrap();
        p.place = Place::Stratum(StratumKind::Street);
        p.pos = ad_at.step(citygen::Dir::S).step(citygen::Dir::S);
        p.body = None;
        tick(&mut st);
        let aside = st.missions.asides.last().expect("an aside");
        assert_eq!(aside.text, "▸ VEX WAS HERE");
        assert_eq!(aside.label, "Vex");
        // Walkers near stare at it.
        assert!(staring(&st, ad_at.step(citygen::Dir::S)));
        // Held again, the hijack lasts with the hold.
        let later = st.net.now(&st) + 5;
        take(
            &mut st,
            citygen::NetNode::City(ad),
            Owner::Building(b),
            later,
        );
        assert_eq!(st.net.hijacks[0].until, later + game::HACK_MINUTES);
        // The hold ends: the ads go back.
        st.net.holds.iter_mut().for_each(|h| h.until = 0);
        tick(&mut st);
        assert!(hijacked_ads(&st).is_empty());
    }

    #[test]
    fn the_lure_sends_walkers() {
        let (mut st, b) = under_kiroflux();
        let (ad, at) = node_cyber(&st, b, NodeKind::AdBoard);
        st.player.as_mut().unwrap().pos = Pos::new(at.x, at.y - 1);
        crack_to_landing(&mut st, HackTarget::Node(ad));
        // On, until someone on foot is within reach of the ads.
        {
            let ads: Vec<Pos> = ad_tiles(&st, b)
                .into_iter()
                .filter(|a| in_front(&st, *a).is_some())
                .collect();
            let city = st.city.as_ref().unwrap();
            let w = st.world.as_mut().unwrap();
            let about = |w: &sim::World| {
                w.people().iter().any(|p| {
                    w.spot_of(p.id).is_none()
                        && w.pos_of(p.id)
                            .is_some_and(|at| ads.iter().any(|a| a.chebyshev(at) <= LURE_REACH))
                })
            };
            for _ in 0..24 * 60 {
                if about(w) {
                    break;
                }
                let t = w.tick() + sim::TICKS_PER_MINUTE;
                w.advance(city, t);
                w.path_some(city, usize::MAX);
            }
            assert!(about(w), "someone passes by in a day");
        }
        st.net.holds.iter_mut().for_each(|h| h.until = u64::MAX);
        let before = st.world.as_ref().unwrap().pending();
        let line = hijack(&mut st, ad, Line::Lure);
        let sent = st.world.as_ref().unwrap().pending() - before;
        assert!(sent > 0, "{line}");
        assert!(line.contains("come to look"), "{line}");
    }

    #[test]
    fn a_decoy_draws_the_ice() {
        let (mut st, _) = under_kiroflux();
        let city = st.city.as_ref().unwrap();
        // A construct of today's kind: a site's gate is its firewall's.
        let (b, gi, ad, at) = city
            .buildings
            .iter()
            .filter(|b| !crate::sitenet::is_site(&st, b.id))
            .find_map(|b| {
                let k = citygen::construct::generate_construct(city, b.id)?;
                let gi = k
                    .guards
                    .iter()
                    .position(|g| g.beat.len() >= 2 && !g.inert)?;
                let (ad, at) = k
                    .nodes
                    .iter()
                    .find(|(n, _)| city.net.nodes[n.index()].kind == NodeKind::AdBoard)?;
                (k.guards[gi].pos.chebyshev(*at) > 3).then_some((b.id, gi, *ad, *at))
            })
            .expect("a construct with a walking piece and an ad board");
        crate::constructs::enter_construct(&mut st, b).unwrap();
        st.player.as_mut().unwrap().pos = at;
        let now = st.net.now(&st);
        take(&mut st, citygen::NetNode::City(ad), Owner::Building(b), now);
        assert_eq!(
            verbs_of(&st, citygen::NetNode::City(ad)),
            vec![Verb::Hijack, Verb::Decoy]
        );
        let done = use_verb(&mut st, 'z').unwrap();
        assert!(
            matches!(done, VerbDone::Said(ref l) if l.starts_with("decoy")),
            "{done:?}"
        );
        assert!(decoy_room(&st, b).contains(&at), "the board's room floods");
        let k = st.constructs.get(b).unwrap();
        st.net.ice_of(k);
        let before = st.constructs.get(b).unwrap().guards[gi].pos.chebyshev(at);
        for _ in 0..3 {
            crate::net::ice_step(&mut st, b, gi, true, false);
        }
        let after = st.constructs.get(b).unwrap().guards[gi].pos.chebyshev(at);
        assert!(after < before, "{before} → {after}");
        assert!(
            matches!(st.net.ice[&b][gi].mode, crate::net::IceMode::Asleep),
            "drawn, not hunting"
        );
        for _ in 0..DECOY_BEATS {
            decoy_beat(&mut st);
        }
        assert!(st.net.decoy.is_none(), "the flood dies down");
        assert!(decoy_room(&st, b).is_empty());
    }

    /// The lamp grid of Kiroflux Mart's district, held from beside it on the Net.
    fn grid_held(st: &mut AppState) -> (NodeId, DistrictId) {
        let city = st.city.as_ref().unwrap();
        let b = city
            .buildings
            .iter()
            .find(|b| b.name.as_deref() == Some("Kiroflux Mart"))
            .unwrap();
        let d = b.district;
        let grid = city
            .net
            .nodes
            .iter()
            .find(|n| {
                n.kind == NodeKind::LampGrid
                    && crate::owners::owner_of(city, n.id) == Some(Owner::District(d))
            })
            .unwrap();
        let (id, at) = (grid.id, grid.cyber);
        st.player.as_mut().unwrap().pos = Pos::new(at.x + 1, at.y);
        crack_to_landing(st, HackTarget::Node(id));
        (id, d)
    }

    /// Advances the world clock to a minute of the day (forward only).
    fn at_minute(st: &mut AppState, minute: u16) {
        let city = st.city.as_ref().unwrap();
        let w = st.world.as_mut().unwrap();
        while w.minute_of_day() != minute {
            let t = w.tick() + sim::TICKS_PER_MINUTE;
            w.advance(city, t);
        }
    }

    #[test]
    fn lights_out_darkens_a_district_for_twenty_minutes() {
        let (mut st, _) = under_kiroflux();
        at_minute(&mut st, 22 * 60);
        let (_, d) = grid_held(&mut st);
        let street = st
            .city
            .as_ref()
            .unwrap()
            .stratum(StratumKind::Street)
            .unwrap();
        let lamp = street
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| street.lamps[*p] > 0 && street.tiles[*p].district == Some(d))
            .expect("a lamp-lit tile in the district");
        let lit = street.emitted(lamp, 22 * 60);
        let done = use_verb(&mut st, 'o').unwrap();
        assert!(
            matches!(done, VerbDone::Said(ref l) if l.starts_with("lights out")),
            "{done:?}"
        );
        let street = st
            .city
            .as_ref()
            .unwrap()
            .stratum(StratumKind::Street)
            .unwrap();
        assert!(
            street.emitted(lamp, 22 * 60) < lit,
            "the lamps' light is gone"
        );
        assert!(!street.lamps_lit(lamp, 22 * 60));
        // Twenty minutes on, they burn again.
        let now = st.net.now(&st);
        st.net.lights_out.insert(d, now);
        tick(&mut st);
        let street = st
            .city
            .as_ref()
            .unwrap()
            .stratum(StratumKind::Street)
            .unwrap();
        assert_eq!(street.emitted(lamp, 22 * 60), lit);
    }

    #[test]
    fn lights_out_is_refused_by_day() {
        let (mut st, _) = under_kiroflux();
        at_minute(&mut st, 12 * 60);
        let (grid, _) = grid_held(&mut st);
        assert_eq!(
            use_verb(&mut st, 'o'),
            Err("the lamps are off by day".to_string())
        );
        assert_eq!(
            verbs_of(&st, citygen::NetNode::City(grid)),
            vec![Verb::LightsOut],
            "not spent"
        );
    }

    #[test]
    fn the_dark_is_not_saved() {
        let (mut st, _) = under_kiroflux();
        at_minute(&mut st, 22 * 60);
        let (_, d) = grid_held(&mut st);
        use_verb(&mut st, 'o').unwrap();
        let street = st
            .city
            .as_ref()
            .unwrap()
            .stratum(StratumKind::Street)
            .unwrap();
        assert!(street.dark.contains(&d));
        let ron = ron::to_string(street).unwrap();
        let back: citygen::Stratum = ron::from_str(&ron).unwrap();
        assert!(back.dark.is_empty());
    }
}
