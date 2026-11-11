//! The site's Net panel beside the tower (look D), and what the tower draws besides the
//! construct: the building and its firewall, the job, the levels, what you hold, and your
//! BODY, where it stands and who is near it (Rulings 19 to 23). Spec
//! `docs/superpowers/specs/2026-11-11-site-net-design.md`, "The look: the open tower".

use crate::app::AppState;
use crate::render::netlook::inside::watch_tint;
use crate::render::netlook::tower::SiteView;
use crate::render::netlook::{AMBER, BLACK_RED, ICE_BLUE, ROOM, WHITE, YOU};
use crate::render::overlay::SIGNAL;
use crate::ui::chrome::{DIM, FAINTER, PANEL_BG_RGB, TEXT};
use crate::ui::hud::RULE_STYLE;
use citygen::sitenet::{ice_on, posted_ice};
use citygen::venue::{VenuePack, VenueType};
use citygen::{
    BuildingId, Construct, FloorPlan, IceGrade, NetDevice, NodeKind, Place, Pos, SiteNode, Size,
    Terrain,
};
use game::{HackTarget, Op};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Words in the panel quieter than its text.
const QUIET: Color = Color::Rgb(138, 147, 163);
/// The longest role a LEVELS row shows.
const ROLE_W: usize = 15;

/// The node your body is jacked in at on site `b`: the terminal or switch it stands at on
/// its floor, on its own side (`at_port`), from where it stands and the construct's
/// devices (Ruling 23).
fn port(st: &AppState, b: BuildingId) -> Option<&NetDevice> {
    let (Place::Floor { building, level }, at) = st.player.as_ref()?.body? else {
        return None;
    };
    let k = st.constructs.site(b).filter(|_| building == b)?;
    // The tile in front of a device is its grid tile, back on the floor.
    k.devices.iter().find(|d| {
        d.node.level == level
            && super::is_port(d.kind)
            && (k.to_floor(d.at)).is_some_and(|(_, front)| super::at_port(d.node.at, front, at))
    })
}

/// The nodes of site `b` with a live hack or hold on them: hacks first, in the order they
/// landed, then holds.
fn worked(st: &AppState, b: BuildingId) -> impl Iterator<Item = SiteNode> + '_ {
    let now = st.net.now(st);
    let hacks = (st.net.live(now)).filter_map(move |h| match h.target {
        HackTarget::Site(s) if s.building == b => Some(s),
        _ => None,
    });
    let holds = (st.net.holds.iter())
        .filter(move |h| now < h.until)
        .filter_map(|h| h.node.site())
        .filter(move |s| s.building == b);
    hacks.chain(holds)
}

/// The cord on level `level` of `k` (Ruling 23): the shortest way over the level's open
/// tiles from the node you jacked in at to you, ends and all, when both are on it.
fn cord(st: &AppState, k: &Construct, level: i32, jacked: Option<Pos>) -> Vec<Pos> {
    let (Some(from), Some(p), Some(l)) = (jacked, st.player.as_ref(), k.net_level(level)) else {
        return Vec::new();
    };
    let here = Place::SiteNet {
        building: k.building,
        level,
    };
    let area = l.area;
    if p.place != here || !area.contains(from) || !area.contains(p.pos) {
        return Vec::new();
    }
    let size = Size {
        w: area.w as u32,
        h: area.h as u32,
    };
    let local = |q: Pos| Pos::new(q.x - area.x, q.y - area.y);
    let open = |q: Pos| {
        (k.tiles.get(q.offset(area.x, area.y))).is_some_and(|t| t.terrain != Terrain::NetWall)
    };
    citygen::pathing::astar(size, local(from), local(p.pos), |_, to| {
        open(to).then_some(1)
    })
    .map(|path| path.into_iter().map(|q| q.offset(area.x, area.y)).collect())
    .unwrap_or_default()
}

/// What the open tower of site `b` draws besides its construct, with level `level` open:
/// what you hold, where you jacked in and the cord from it, the firewall's locks, the levels
/// you can reach, and from the site's Net each level's fold, the lifts through its slab and
/// its turnstiles. Worked out each frame from the kept Net (`net_of`) and the construct.
pub fn site_view(st: &AppState, b: BuildingId, level: i32) -> SiteView {
    let Some(k) = st.constructs.site(b) else {
        return SiteView {
            open: level,
            ..SiteView::default()
        };
    };
    let mut held: Vec<Pos> = worked(st, b)
        .filter_map(|n| k.device(n).map(|d| d.at))
        .collect();
    held.sort_by_key(|p| (p.y, p.x));
    held.dedup();
    let jacked = port(st, b).map(|d| d.at);
    let mut view = SiteView {
        open: level,
        cord: cord(st, k, level, jacked),
        jacked,
        broken: super::broken(st, b),
        reach: vec![level],
        ..SiteView::default()
    };
    if let Some(net) = super::net_of(st, b) {
        view.reach = (net.levels.iter())
            .map(|l| l.level)
            .filter(|l| net.reaches(level, *l))
            .collect();
        for l in &net.levels {
            view.folds.push((l.level, l.fold));
            view.lifts.extend(l.lifts.iter().map(|x| (l.level, *x)));
            for &(at, switch) in &l.turnstiles {
                let Some(tile) = k.on_level(l.level, at) else {
                    continue;
                };
                let open = switch
                    .map(|s| SiteNode {
                        building: b,
                        level: l.level,
                        at: s,
                    })
                    .and_then(|s| k.device(s))
                    .is_some_and(|d| held.contains(&d.at));
                view.gates.push((tile, open));
            }
        }
    }
    view.held = held;
    view
}

/// The venue type site `b` is laid out as: its plan's, else the type for its kind.
fn site_type(st: &AppState, b: BuildingId) -> Option<&'static VenueType> {
    let pack = VenuePack::builtin();
    match (st.interiors.plan_of(b)).and_then(|p| p.venue.as_deref()) {
        Some(v) => pack.by_id(&v.type_id),
        None => pack.for_kind(st.city.as_ref()?.building_opt(b)?.kind),
    }
}

/// The job's line (Ruling 20): the live copy's objective, `{objective name} · {level}`, and
/// under it, when that level is gapped, `{level} has its own Net: jack in there`. None on a
/// plain site.
pub fn job_lines(st: &AppState, b: BuildingId) -> Vec<String> {
    let Some(layer) = super::layer_of(st, b) else {
        return Vec::new();
    };
    let Some(def) = site_type(st, b).and_then(|t| t.objectives.get(layer.inputs.objective)) else {
        return Vec::new();
    };
    let level = layer.objective.level;
    let label = FloorPlan::label(level);
    let mut out = vec![format!("{} · {label}", def.name)];
    if super::net_of(st, b).is_some_and(|n| n.gap == Some(level)) {
        out.push(format!("{label} has its own Net: jack in there"));
    }
    out
}

/// The panel's width: the sidebar's.
fn panel_w() -> usize {
    crate::ui::sidebar::width(0) as usize
}

/// `s` cut to `n` characters.
fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn span(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg))
}

fn bold(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(
        s.into(),
        Style::default().fg(fg).add_modifier(Modifier::BOLD),
    )
}

fn rule() -> Line<'static> {
    Line::from(Span::styled("─".repeat(panel_w()), RULE_STYLE))
}

/// A grade's colour in the panel: the ICE's own, without its pulse.
fn ice_text(g: IceGrade) -> Color {
    watch_tint(g).0.to_color()
}

/// What a hack or hold on a node does, as HELD says it.
fn held_word(kind: NodeKind, op: Op) -> Option<&'static str> {
    Some(match op {
        Op::Crack if kind == NodeKind::Server => "root",
        Op::Crack => "cracked",
        Op::Spoof => "spoofed",
        Op::Copy => "copied",
        Op::Blind => "blind",
        Op::Backdoor => "root",
        _ => return None,
    })
}

/// The site's Net panel (look D), HERE's lines on its Net: the building, its security and
/// ICE, its firewall's locks; the job (Ruling 20); LEVELS, a row each with its nodes, its
/// ICE, the server, the vault and you, or why it's out of reach; HELD, what you hold and
/// what each does; BODY, where your body stands and who is near it. Counts in digits.
pub fn panel_lines(st: &AppState, b: BuildingId) -> Vec<Line<'static>> {
    let (Some(city), Some(k)) = (st.city.as_ref(), st.constructs.site(b)) else {
        return Vec::new();
    };
    let Some(building) = city.building_opt(b) else {
        return Vec::new();
    };
    let w = panel_w();
    let here = match st.player.as_ref().map(|p| p.place) {
        Some(Place::SiteNet { building, level }) if building == b => Some(level),
        _ => None,
    };
    let net = super::net_of(st, b);
    let mut out: Vec<Line<'static>> = Vec::new();
    // The building.
    let kind = citygen::kind_label(city, b);
    let name = building.name.as_deref().unwrap_or(kind).to_uppercase();
    let type_name = site_type(st, b).map_or(kind, |t| t.name.as_str());
    let name = cut(&name, w - 1);
    let mut head = vec![Span::raw(" "), bold(name.clone(), BLACK_RED.to_color())];
    if name.chars().count() + 4 + type_name.chars().count() <= w {
        head.push(span(format!(" · {type_name}"), TEXT));
    }
    out.push(Line::from(head));
    let security = super::security(st, b);
    let mut sec = vec![
        span(" SEC ", TEXT),
        bold(security.to_string(), WHITE.to_color()),
    ];
    let ice = posted_ice(k);
    if let Some(g) = ice.g {
        sec.push(span(format!(" · ICE {}", g.label()), TEXT));
    }
    if let Some(g) = ice.vault {
        sec.push(span(format!(" · vault {}", g.label()), TEXT));
    }
    out.push(Line::from(sec));
    let broken = super::broken(st, b);
    let down = broken.iter().filter(|x| **x).count();
    let mut fw = vec![span(" FIREWALL ", BLACK_RED.to_color())];
    for x in &broken {
        fw.push(match x {
            true => bold("▯", SIGNAL.to_color()),
            false => bold("▮", BLACK_RED.to_color()),
        });
    }
    fw.push(span(
        format!(" {down}/{}", broken.len()),
        BLACK_RED.to_color(),
    ));
    out.push(Line::from(fw));
    out.push(rule());
    // The job.
    let job = job_lines(st, b);
    if let Some((first, rest)) = job.split_first() {
        out.push(Line::from(vec![
            span(" JOB ", DIM),
            span(cut(first, w - 5), AMBER.to_color()),
        ]));
        for line in rest {
            out.push(Line::from(span(
                format!("     {}", cut(line, w - 5)),
                QUIET,
            )));
        }
        out.push(rule());
    }
    // A row a level.
    out.push(Line::from(span(" LEVELS", DIM)));
    let from = here.unwrap_or(0);
    let reaches = |l: i32| net.as_ref().is_none_or(|n| n.reaches(from, l));
    for l in &k.levels {
        let yours = here == Some(l.level);
        let bg = match yours {
            true => PANEL_BG_RGB.mix(ROOM, 0.12).to_color(),
            false => Color::Reset,
        };
        let on = |s: Span<'static>| match yours {
            true => s.patch_style(Style::default().bg(bg)),
            false => s,
        };
        let mut row = vec![
            span(" ", TEXT),
            bold(if yours { "▸" } else { " " }, ROOM.to_color()),
            span(" ", TEXT),
        ];
        let reach = reaches(l.level);
        let label = format!("{:<4}", FloorPlan::label(l.level));
        row.push(bold(label, if reach { WHITE.to_color() } else { FAINTER }));
        let role = format!("{:<w$}", cut(&l.name, ROLE_W), w = ROLE_W + 1);
        let role_fg = match (yours, reach) {
            (true, _) => TEXT,
            (false, true) => QUIET,
            (false, false) => FAINTER,
        };
        row.push(span(role, role_fg));
        if !reach {
            row.push(match l.gapped {
                true => bold("AIR GAP", BLACK_RED.to_color()),
                false => span("out of reach", FAINTER),
            });
        } else {
            let ours = (k.devices.iter()).filter(|d| d.node.level == l.level);
            let n = ours.clone().filter(|d| d.kind != NodeKind::Hub).count();
            row.push(span(format!("▪{n:<3}"), ICE_BLUE.to_color()));
            match ice_on(k, l.level) {
                Some(g) => row.push(span(format!(" ◈ {:<6}", g.label()), ice_text(g))),
                None => row.push(span(" ".repeat(9), TEXT)),
            }
            let mut marks = vec![span(" ", TEXT)];
            if ours.clone().any(|d| d.kind == NodeKind::Server) {
                marks.push(bold("◆", WHITE.to_color()));
            }
            if ours.clone().any(|d| d.kind == NodeKind::DataVault) {
                marks.push(bold("⌗", AMBER.to_color()));
            }
            if yours {
                marks.push(bold("@", YOU.to_color()));
            }
            if marks.len() > 1 {
                row.extend(marks);
            }
        }
        let used: usize = row.iter().map(|s| s.content.chars().count()).sum();
        row.push(span(" ".repeat(w.saturating_sub(used)), TEXT));
        out.push(Line::from(row.into_iter().map(on).collect::<Vec<_>>()));
    }
    out.push(rule());
    // What you hold, a row a node and what each does.
    out.push(Line::from(span(" HELD", DIM)));
    let now = st.net.now(st);
    // A row a node and what it does: two nodes of one name are two rows.
    let mut rows: Vec<(SiteNode, char, String, &'static str)> = Vec::new();
    for d in &k.devices {
        let mut words: Vec<&'static str> = Vec::new();
        if let Some(h) = crate::holds::held(st, citygen::NetNode::Site(d.node), now) {
            words.push(match h.used.contains(&crate::holds::Verb::Loop) {
                true => "looped",
                false => "held",
            });
        }
        let hacks = (st.net.live(now)).filter(|h| h.target == HackTarget::Site(d.node));
        words.extend(hacks.filter_map(|h| held_word(d.kind, h.op)));
        let glyph = match d.kind {
            NodeKind::DoorLock => '▯',
            NodeKind::Terminal => '◇',
            NodeKind::Server => '◆',
            NodeKind::DataVault => '⌗',
            _ => '▪',
        };
        for word in words {
            if !rows.iter().any(|r| r.0 == d.node && r.3 == word) {
                rows.push((d.node, glyph, d.name.clone(), word));
            }
        }
    }
    if rows.is_empty() {
        out.push(Line::from(span("  none", FAINTER)));
    }
    for (_, glyph, name, word) in rows {
        let room = w.saturating_sub(4 + 3 + word.chars().count());
        out.push(Line::from(vec![
            span("  ", TEXT),
            bold(glyph.to_string(), SIGNAL.to_color()),
            span(format!(" {}", cut(&name, room)), TEXT),
            span(format!(" · {word}"), SIGNAL.to_color()),
        ]));
    }
    out.push(rule());
    // Your body.
    let body = body_lines(st);
    let mut body = body.iter();
    let first = body.next().map_or(String::new(), |s| cut(s, w - 6));
    out.push(Line::from(vec![span(" BODY ", DIM), span(first, TEXT)]));
    for line in body {
        out.push(Line::from(span(format!("   {}", cut(line, w - 3)), QUIET)));
    }
    out
}

/// BODY: where your body stands and who is near it (Ruling 19). On a site's floor,
/// `L · at the lobby kiosk`, then `3 guards on L · nearest 4 tiles` (the guards posted to
/// the building on that floor, not down) or `nobody on L`; at home `home · at the rig`; at
/// a street port `{district} · at the port`. None while you're not jacked in.
pub fn body_lines(st: &AppState) -> Vec<String> {
    let Some((place, at)) = st.player.as_ref().and_then(|p| p.body) else {
        return Vec::new();
    };
    if st.home.is_some_and(|h| h.place() == place) {
        return vec!["home · at the rig".to_string()];
    }
    match place {
        Place::Stratum(_) => {
            let district = (st.city.as_ref())
                .map(|c| crate::net::district_name(c, at))
                .unwrap_or_default();
            vec![format!("{district} · at the port")]
        }
        Place::Floor { building: b, level } => {
            let label = FloorPlan::label(level);
            let here = match port(st, b) {
                Some(d) => format!("{label} · at the {}", d.name),
                None => label.clone(),
            };
            let guards: Vec<i32> = (st.foes.iter())
                .filter(|f| !f.down && f.place == place)
                .filter(|f| f.post.as_ref().is_some_and(|p| p.building == b))
                .map(|f| f.pos.chebyshev(at))
                .collect();
            let near = match guards.iter().min() {
                None => format!("nobody on {label}"),
                Some(d) => format!(
                    "{} {} on {label} · nearest {d} {}",
                    guards.len(),
                    if guards.len() == 1 { "guard" } else { "guards" },
                    if *d == 1 { "tile" } else { "tiles" },
                ),
            };
            vec![here, near]
        }
        Place::Construct(_) | Place::SiteNet { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;
    use crate::foes::{Foe, Post};
    use citygen::{NodeKind, Place, Pos, StratumKind};
    use game::Op;

    #[test]
    fn the_body_line_says_where_it_stands_and_who_is_near() {
        let (mut st, b) = office_copy(190);
        stand_beside_the_kiosk(&mut st, b);
        crate::net::jack_in_here(&mut st).unwrap();
        let lines = body_lines(&st);
        assert!(
            lines[0].starts_with("L · at the ") && lines[0].contains("kiosk"),
            "{lines:?}"
        );
        assert!(
            lines[1].contains(" guards on L · nearest ") || lines[1] == "nobody on L",
            "{lines:?}"
        );
    }

    #[test]
    fn a_body_behind_a_terminals_wall_is_not_at_it() {
        let (mut st, b) = office_copy(190);
        let terminal = stand_behind_a_terminal(&mut st, b);
        let behind = st.player.as_ref().unwrap().pos;
        let front = crate::sitenet::net_of(&st, b)
            .unwrap()
            .node(terminal)
            .unwrap()
            .net;
        let floor = Place::Floor {
            building: b,
            level: terminal.level,
        };
        crate::constructs::land(&mut st, floor, front);
        crate::net::jack_in_here(&mut st).unwrap();
        let label = citygen::FloorPlan::label(terminal.level);
        assert!(
            body_lines(&st)[0].starts_with(&format!("{label} · at the ")),
            "in front"
        );
        st.player.as_mut().unwrap().body = Some((floor, behind));
        assert_eq!(body_lines(&st)[0], label, "behind its wall: at nothing");
    }

    #[test]
    fn who_is_near_counts_the_guards_posted_on_the_bodys_floor() {
        let (mut st, b) = office_copy(190);
        stand_beside_the_kiosk(&mut st, b);
        let body = st.player.as_ref().unwrap().pos;
        crate::net::jack_in_here(&mut st).unwrap();
        let on = |level: i32| Place::Floor { building: b, level };
        let guard = |building, level: i32, dx: i32| Foe {
            post: Some(Post {
                building,
                zone: 0,
                patrol: Vec::new(),
            }),
            ..Foe::goon(
                Place::Floor { building, level },
                Pos::new(body.x + dx, body.y),
            )
        };
        st.foes.clear();
        assert_eq!(body_lines(&st)[1], "nobody on L");
        st.foes.push(guard(b, 0, 3));
        assert_eq!(body_lines(&st)[1], "1 guard on L · nearest 3 tiles");
        st.foes.push(guard(b, 0, -5));
        // Not counted: one down, one on another floor, a goon with no post.
        let mut down = guard(b, 0, 1);
        down.down = true;
        st.foes.push(down);
        st.foes.push(guard(b, 1, 1));
        st.foes.push(Foe::goon(on(0), Pos::new(body.x + 1, body.y)));
        assert_eq!(body_lines(&st)[1], "2 guards on L · nearest 3 tiles");
    }

    #[test]
    fn a_body_at_the_rig_or_a_street_port_says_so() {
        let (mut st, _) = office_copy(190);
        assert!(body_lines(&st).is_empty(), "not jacked in");
        stand_at_the_rig(&mut st);
        crate::net::jack_in_here(&mut st).unwrap();
        assert_eq!(body_lines(&st), vec!["home · at the rig".to_string()]);
        crate::net::jack_out(&mut st).unwrap();
        // Beside a street Jack.
        let city = st.city.clone().unwrap();
        let street = StratumKind::Street;
        let at = (city.connectors.iter())
            .filter(|k| k.kind == citygen::ConnectorKind::Jack && k.serves.contains(&street))
            .filter(|k| city.walkable(StratumKind::Net, k.pos))
            .find_map(|k| (k.pos.neighbors8()).find(|q| city.walkable(street, *q)))
            .expect("a street Jack");
        crate::constructs::land(&mut st, Place::Stratum(street), at);
        crate::net::jack_in_here(&mut st).unwrap();
        let district = crate::net::district_name(&city, at);
        assert_eq!(body_lines(&st), vec![format!("{district} · at the port")]);
    }

    #[test]
    fn the_panel_says_the_building_the_firewall_and_the_levels_in_digits() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        let lines: Vec<String> = panel_lines(&st, b).iter().map(|l| l.to_string()).collect();
        assert!(lines.iter().any(|l| l.contains("SEC 190")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.trim() == "FIREWALL ▮▮▮ 0/3"));
        assert!(
            (lines.iter()).any(|l| l.contains("B1 has its own Net: jack in there")),
            "{lines:#?}"
        );
        assert!(lines.iter().any(|l| l.contains("LEVELS")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("B1") && l.contains("AIR GAP"))
        );
        assert!(lines.iter().any(|l| l.contains("BODY")));
        // Every line fits the panel: none wraps.
        let w = crate::ui::sidebar::width(0) as usize;
        assert!(lines.iter().all(|l| l.chars().count() <= w), "{lines:#?}");
    }

    #[test]
    fn the_panel_lists_what_you_hold_and_the_broken_locks() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        let door = device(&st, b, NodeKind::DoorLock);
        hack(&mut st, door.node, NodeKind::DoorLock, Op::Spoof);
        let now = st.net.now(&st);
        st.net.break_lock(b, 0, now);
        let lines: Vec<String> = panel_lines(&st, b).iter().map(|l| l.to_string()).collect();
        assert!(
            lines.iter().any(|l| l.trim() == "FIREWALL ▯▮▮ 1/3"),
            "{lines:#?}"
        );
        let held = format!("▯ {} · spoofed", door.name);
        assert!(lines.iter().any(|l| l.contains(&held)), "{lines:#?}");
        let view = site_view(&st, b, 0);
        assert!(view.held.contains(&door.at));
        assert_eq!(view.broken, vec![true, false, false]);
    }

    #[test]
    fn held_lists_two_nodes_of_one_name_apart() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        // Two door locks under one name, both spoofed: a row each, as the tower counts
        // each node it holds.
        let mut k = st.constructs.site(b).unwrap().clone();
        let doors: Vec<usize> = (k.devices.iter().enumerate())
            .filter(|(_, d)| d.kind == NodeKind::DoorLock)
            .map(|(i, _)| i)
            .take(2)
            .collect();
        assert_eq!(doors.len(), 2, "two door locks");
        let name = k.devices[doors[0]].name.clone();
        k.devices[doors[1]].name = name.clone();
        let nodes = doors.iter().map(|i| k.devices[*i].node).collect::<Vec<_>>();
        st.constructs.put_site(k);
        for node in nodes {
            hack(&mut st, node, NodeKind::DoorLock, Op::Spoof);
        }
        let lines: Vec<String> = panel_lines(&st, b).iter().map(|l| l.to_string()).collect();
        let row = format!("▯ {name} · spoofed");
        assert_eq!(
            lines.iter().filter(|l| l.contains(&row)).count(),
            2,
            "{lines:#?}"
        );
    }

    #[test]
    fn a_plain_site_has_no_job_line() {
        let (st, b) = office_plain();
        assert!(job_lines(&st, b).is_empty());
        let (st, b) = office_copy(190);
        let job = job_lines(&st, b);
        assert!(job[0].ends_with(" · B1"), "{job:?}");
        assert_eq!(job[1], "B1 has its own Net: jack in there");
    }

    #[test]
    fn the_view_reaches_the_riser_and_cords_you_to_your_port() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        let gap = crate::sitenet::net_of(&st, b).unwrap().gap.unwrap();
        let view = site_view(&st, b, 0);
        assert_eq!(view.open, 0);
        assert!(view.reach.contains(&0) && !view.reach.contains(&gap));
        assert_eq!(view.jacked, None, "jacked in from the street");
        assert!(view.cord.is_empty());
        // Jacked in at the kiosk, then walked off: the cord runs from it to you.
        crate::net::jack_out(&mut st).unwrap();
        stand_beside_the_kiosk(&mut st, b);
        crate::net::jack_in_here(&mut st).unwrap();
        let kiosk = st.player.as_ref().unwrap().pos;
        let view = site_view(&st, b, 0);
        assert_eq!(view.jacked, Some(kiosk));
        on_hub(&mut st, b, 0);
        let you = st.player.as_ref().unwrap().pos;
        let view = site_view(&st, b, 0);
        assert_eq!(view.cord.first(), Some(&kiosk));
        assert_eq!(view.cord.last(), Some(&you));
        assert!(view.cord.windows(2).all(|w| w[0].manhattan(w[1]) == 1));
    }
}
