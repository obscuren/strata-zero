//! The rig's panel beside its room (the mock's `panel()`, round 5): the rack, the
//! firewall, the lines and what each holds, the reader's slots, the seats' jobs, what is
//! under you and where your body is.

use super::{clip, mastiff, reader, room, sled_name, span};
use crate::app::AppState;
use crate::backdoors::Kind;
use crate::rig::jobs::{JobKind, sift_left, target, up_label};
use game::SocketKind;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const AMBER: Color = Color::Rgb(0xff, 0xb0, 0x00);
const FW: Color = Color::Rgb(0xff, 0x28, 0x48);
const LABEL: Color = Color::Rgb(0xa0, 0xaa, 0xb9);
const TEXT: Color = Color::Rgb(0xcd, 0xd4, 0xde);
const DIM: Color = Color::Rgb(0x8a, 0x93, 0xa3);
const FAINT: Color = Color::Rgb(0x4a, 0x52, 0x5e);
const RELAY: Color = Color::Rgb(0x50, 0xe6, 0x78);
const ICE_BLUE: Color = Color::Rgb(0x9f, 0xe6, 0xff);
const CORE: Color = Color::Rgb(0x00, 0xe5, 0xff);

fn width() -> usize {
    crate::ui::sidebar::width(0) as usize
}

fn sp(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(fg))
}

fn bold(s: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(
        s.into(),
        Style::default().fg(fg).add_modifier(Modifier::BOLD),
    )
}

fn rule() -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(width()),
        crate::ui::hud::RULE_STYLE,
    ))
}

/// A row: its marks and name on the left, `tail` against the right edge.
fn row(mut left: Vec<Span<'static>>, tail: Option<Span<'static>>) -> Line<'static> {
    let used: usize = left.iter().map(|s| s.content.chars().count()).sum();
    if let Some(tail) = tail {
        let t = tail.content.chars().count();
        let pad = width().saturating_sub(1 + used + t).max(1);
        left.push(Span::raw(" ".repeat(pad)));
        left.push(tail);
    }
    Line::from(left)
}

/// What a backdoor on `b` is called in the panel: `key`, `site` (a Net backdoor on a
/// building with a Net of its own) or `net`.
fn kind_word(st: &AppState, b: citygen::BuildingId, kind: &Kind) -> &'static str {
    match kind {
        Kind::Key { .. } => "key",
        Kind::Net { .. } if crate::sitenet::is_site(st, b) => "site",
        Kind::Net { .. } => "net",
    }
}

/// The name of the sled fitted in the rack's bay of `kind`, as the panel says it.
fn sled(st: &AppState, kind: SocketKind) -> Option<String> {
    (st.character.rig_fitted(&st.content).into_iter())
        .filter(|(k, _)| *k == kind)
        .find_map(|(_, id)| st.content.item(&id?).map(|i| sled_name(&i.name)))
}

/// The panel: the rig as it stands, and under you what you stand at.
pub fn panel_lines(st: &AppState) -> Vec<Line<'static>> {
    let rig = crate::rig::of(st);
    let now = st.net.now(st);
    let mut out: Vec<Line<'static>> = Vec::new();
    let w = width();
    out.push(Line::from(vec![
        sp(" ", TEXT),
        bold("ZENBO", AMBER),
        sp(" · your rig", TEXT),
    ]));
    let rack = st.content.item(&st.character.rig_frame());
    let name = rack.map_or("rack", |i| i.name.as_str());
    let bays = rack.map_or(0, |i| i.sockets.len());
    out.push(Line::from(sp(
        format!(" {} · {bays} bays", clip(name, w.saturating_sub(12))),
        DIM,
    )));
    let mut fw = vec![sp(" ", TEXT), bold("FIREWALL", FW), sp(" ", TEXT)];
    fw.extend((0..3).map(|n| sp(if n < rig.firewall { "▮" } else { "▯" }, FW)));
    let word = sled(st, SocketKind::Firewall)
        .unwrap_or_default()
        .to_lowercase();
    if !word.is_empty() {
        fw.push(sp(format!(" {word}"), DIM));
    }
    out.push(Line::from(fw));
    out.push(rule());
    // The lines.
    let lines = super::door_lines(st);
    let held = lines.iter().flatten().count();
    out.push(Line::from(vec![
        sp(" ", TEXT),
        bold("LINES", LABEL),
        sp(format!(" {held}/{}", lines.len()), TEXT),
        sp(format!(" · hold {} days", rig.hold_days), DIM),
    ]));
    let running = &st.backdoors.jobs.running;
    for line in &lines {
        let live = line.and_then(|b| st.backdoors.live(b).map(|d| (b, d)));
        let Some((b, d)) = live else {
            out.push(row(
                vec![sp(" ", TEXT), bold("▯", FAINT), sp(" free line", FAINT)],
                None,
            ));
            continue;
        };
        let kept = running.iter().any(|j| j.kind == JobKind::Keepalive(b));
        let kind = kind_word(st, b, &d.kind);
        let tail = match kept {
            true => sp(format!("{kind} · held"), CORE),
            false => sp(format!("{kind} · {}", span(d.left(now))), DIM),
        };
        let name = clip(&crate::net::building_name(st, b), w.saturating_sub(16));
        out.push(row(
            vec![
                sp(" ", TEXT),
                bold("▯", RELAY),
                sp(format!(" {name}"), TEXT),
            ],
            Some(tail),
        ));
    }
    out.push(row(
        vec![sp(" ", TEXT), bold("▸", ICE_BLUE), sp(" the grid", TEXT)],
        Some(sp("your block", DIM)),
    ));
    out.push(rule());
    // The reader.
    let slots = st.character.reader_slots(&st.content);
    let stacks: Vec<_> = (0..slots)
        .map(|i| st.character.reader.get(i).and_then(|s| s.as_ref()))
        .collect();
    out.push(Line::from(vec![
        sp(" ", TEXT),
        bold("READER", LABEL),
        sp(
            format!(" {}/{slots}", stacks.iter().flatten().count()),
            TEXT,
        ),
    ]));
    for (i, stack) in stacks.into_iter().enumerate() {
        let Some(stack) = stack else {
            out.push(Line::from(sp(" · free slot", FAINT)));
            continue;
        };
        let holds = reader::holds(st, &stack.item);
        let (glyph, colour) = match holds {
            Some(h) => reader::glyph(h),
            None => ('·', DIM),
        };
        let item = st.content.item(&stack.item);
        let name = item.map_or(stack.item.0.as_str(), |i| i.name.as_str());
        // The black chip's tail is its ICE, in red: asleep, or awake off it.
        let tail = match holds {
            Some(reader::Holds::Black) if mastiff::awake_on(st, i) => sp("awake", FW),
            Some(reader::Holds::Black) => sp(reader::summon_word(st, stack), FW),
            _ => sp(reader::summon_word(st, stack), DIM),
        };
        out.push(row(
            vec![
                sp(" ", TEXT),
                bold(glyph.to_string(), colour),
                sp(format!(" {}", clip(name, w.saturating_sub(18))), TEXT),
            ],
            Some(tail),
        ));
    }
    out.push(rule());
    // The seats: the jobs in them, and each free one in a fight with MASTIFF.
    let fighting = mastiff::fighting_seats(st);
    let core = sled(st, SocketKind::Core);
    let mut head = vec![
        sp(" ", TEXT),
        bold("SEATS", LABEL),
        sp(
            format!(" {}/{}", running.len() + fighting.len(), rig.seats),
            TEXT,
        ),
    ];
    if let Some(core) = core {
        head.push(sp(format!(" · {core}"), DIM));
    }
    out.push(Line::from(head));
    for job in running {
        let on = match &job.kind {
            JobKind::Sift { shard } => st
                .content
                .item(shard)
                .map_or_else(|| target(st, &job.kind), |i| i.name.clone()),
            JobKind::Sniff(b) | JobKind::Keepalive(b) => crate::net::building_name(st, *b),
            JobKind::Rent => target(st, &job.kind),
        };
        let tail = match &job.kind {
            JobKind::Sift { .. } => {
                let (left, _) = sift_left(job, rig.speed, now).unwrap_or((0, 1));
                span(left)
            }
            JobKind::Keepalive(_) => "held".to_string(),
            JobKind::Sniff(_) | JobKind::Rent => up_label(now.saturating_sub(job.started)),
        };
        out.push(row(
            vec![
                sp(" ", TEXT),
                bold("◉", CORE),
                sp(
                    format!(
                        " {}",
                        clip(&format!("{} · {on}", job.kind.name()), w.saturating_sub(10))
                    ),
                    TEXT,
                ),
            ],
            Some(sp(tail, DIM)),
        ));
    }
    for n in &fighting {
        out.push(row(
            vec![
                sp(" ", TEXT),
                bold("◉", CORE),
                sp(" attack · MASTIFF", TEXT),
            ],
            Some(sp(mastiff::seat_word(st, *n), DIM)),
        ));
    }
    out.push(rule());
    out.extend(here_lines(st));
    // Your body back home, and what the bites have cost it.
    let mut body = vec![sp(" ", TEXT), bold("BODY", LABEL), sp(" at home", DIM)];
    let (hp, max) = (st.character.hp, st.character.hp_max(&st.content));
    if hp < max {
        body.push(sp(" · ", DIM));
        body.push(sp(format!("HP {hp}/{max}"), TEXT));
    }
    out.push(Line::from(body));
    out
}

/// MASTIFF awake, as the panel reads it under you wherever you stand in the room: its
/// coat plate by plate, its core, the bite it has marked, its leash.
fn fight_lines(st: &AppState) -> Option<Vec<Line<'static>>> {
    let m = room(st)?.mastiff.as_ref().filter(|_| mastiff::awake(st))?;
    let coat = m.coat();
    let core = match coat {
        0 => sp(
            format!("{} {}", m.core, if m.core == 1 { "hit" } else { "hits" }),
            TEXT,
        ),
        _ => sp("sealed under the coat", DIM),
    };
    let mut out = vec![
        Line::from(vec![
            sp(" ", TEXT),
            bold("◆ MASTIFF", FW),
            sp(" · black ICE", DIM),
        ]),
        Line::from(vec![
            sp("   COAT  ", LABEL),
            sp("▮".repeat(coat), FW),
            sp("▯".repeat(8 - coat), FAINT),
            sp(format!(" {coat}/8"), TEXT),
        ]),
        Line::from(vec![sp("   CORE  ", LABEL), core]),
    ];
    if m.bite.is_some() {
        out.push(Line::from(vec![
            sp("   BITE  ", LABEL),
            sp("next turn · the × tiles", TEXT),
        ]));
    }
    out.push(Line::from(vec![
        sp("   LEASH ", LABEL),
        sp("the red ground", DIM),
    ]));
    Some(out)
}

/// What is under you: MASTIFF while it's awake; a slot's thing and what summoning does,
/// a door's way out, the trunk, the terminal. Nothing when you stand at none.
pub fn here_lines(st: &AppState) -> Vec<Line<'static>> {
    if let Some(fight) = fight_lines(st) {
        return fight;
    }
    let (Some(room), Some(p)) = (room(st), st.player.as_ref()) else {
        return Vec::new();
    };
    let pos = p.pos;
    let near = |q: citygen::Pos| pos.chebyshev(q) <= 1;
    if let Some(i) = room
        .slots
        .iter()
        .position(|r| super::layout::at_box(pos, *r))
    {
        let Some(stack) = st.character.reader.get(i).and_then(|s| s.as_ref()) else {
            return vec![Line::from(sp(" · free slot", FAINT))];
        };
        let (glyph, colour) = match reader::holds(st, &stack.item) {
            Some(h) => reader::glyph(h),
            None => ('·', DIM),
        };
        let name = st
            .content
            .item(&stack.item)
            .map_or(stack.item.0.clone(), |i| i.name.clone());
        let word = match reader::holds(st, &stack.item) {
            Some(reader::Holds::Chip) => {
                let credits = stack.copy.as_ref().map_or(0, |k| k.credits);
                format!("¤{}", crate::ui::commas(credits))
            }
            _ => reader::summon_word(st, stack),
        };
        if reader::holds(st, &stack.item) == Some(reader::Holds::Locked) {
            let credits = stack.copy.as_ref().map_or(0, |k| k.credits);
            let cracked = stack.copy.as_ref().map_or(0, |k| k.cracked);
            let left = reader::locks(credits).saturating_sub(cracked);
            let chance = match super::crack::crack_chance(st, i) {
                Ok(c) => format!("crack {c}%"),
                Err(why) => why,
            };
            return vec![
                Line::from(vec![
                    sp(" ", TEXT),
                    bold(glyph.to_string(), colour),
                    sp(format!(" {name}"), TEXT),
                    sp(format!(" · ¤{} inside", crate::ui::commas(credits)), DIM),
                ]),
                Line::from(sp(
                    format!(
                        "   {left} {} · {chance} · space",
                        if left == 1 { "lock" } else { "locks" }
                    ),
                    DIM,
                )),
            ];
        }
        return vec![
            Line::from(vec![
                sp(" ", TEXT),
                bold(glyph.to_string(), colour),
                sp(format!(" {name}"), TEXT),
                sp(format!(" · {word}"), DIM),
            ]),
            Line::from(sp("   summon · space", DIM)),
        ];
    }
    if let Some(i) = room.doors.iter().position(|d| near(*d)) {
        let live = super::door_lines(st)
            .get(i)
            .copied()
            .flatten()
            .and_then(|b| st.backdoors.live(b).map(|d| (b, d)));
        let Some((b, d)) = live else {
            return vec![Line::from(vec![
                sp(" ", TEXT),
                bold("▯", FAINT),
                sp(" free line", FAINT),
            ])];
        };
        let kind = kind_word(st, b, &d.kind);
        let left = span(d.left(st.net.now(st)));
        return vec![
            Line::from(vec![
                sp(" ", TEXT),
                bold("▯", RELAY),
                sp(format!(" {}", crate::net::building_name(st, b)), TEXT),
                sp(format!(" · {kind} · {left}"), DIM),
            ]),
            Line::from(sp("   step through to its gate", DIM)),
        ];
    }
    if near(room.trunk) {
        return vec![Line::from(vec![
            sp(" ", TEXT),
            bold("▸", ICE_BLUE),
            sp(" the grid", TEXT),
            sp(" · your block", DIM),
        ])];
    }
    if near(room.terminal) {
        return vec![
            Line::from(vec![
                sp(" ", TEXT),
                bold("◇", RELAY),
                sp(" TERMINAL", TEXT),
                sp(" · your rig's shell", DIM),
            ]),
            Line::from(sp("   r opens it from anywhere here", DIM)),
            Line::from(sp("   yours: no crack, no hold to run out", DIM)),
        ];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rigroom::testutil::the_mocks_rig;

    fn text(lines: &[Line<'static>]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn the_panel_reads_the_rig() {
        let (st, _) = the_mocks_rig();
        let t = text(&panel_lines(&st));
        let has = |s: &str| t.iter().any(|l| l.contains(s));
        assert!(has("ZENBO · your rig"), "{t:#?}");
        assert!(has("Civitas cabinet · 4 bays"), "{t:#?}");
        assert!(has("FIREWALL ▮▯▯ sandbag"), "{t:#?}");
        assert!(has("LINES 3/5 · hold 5 days"), "{t:#?}");
        assert!(has("net · held"), "keepalive holds the first: {t:#?}");
        assert!(has("net · "), "{t:#?}");
        assert!(has("key · "), "{t:#?}");
        assert_eq!(t.iter().filter(|l| l.contains("free line")).count(), 2);
        assert!(has("the grid") && has("your block"), "{t:#?}");
        assert!(has("READER 1/3"), "{t:#?}");
        assert!(has("Credchip") && has("+¤340"), "{t:#?}");
        assert_eq!(t.iter().filter(|l| l.contains("free slot")).count(), 2);
        assert!(has("SEATS 2/2 · Tinman 2X"), "{t:#?}");
        assert!(has("◉ sift · "), "{t:#?}");
        assert!(has("◉ keepalive · "), "{t:#?}");
        assert!(t.last().unwrap().contains("BODY at home"), "{t:#?}");
        let w = width();
        assert!(t.iter().all(|l| l.chars().count() <= w), "{t:#?}");
    }

    #[test]
    fn here_is_blank_in_the_hall_and_says_a_slot_a_door_and_the_trunk() {
        let (mut st, [kiroflux, ..]) = the_mocks_rig();
        crate::rigroom::refresh(&mut st);
        let room = room(&st).unwrap().clone();
        let put = |st: &mut AppState, p: citygen::Pos| st.player.as_mut().unwrap().pos = p;
        assert!(here_lines(&st).is_empty(), "the landing");
        put(&mut st, crate::rigroom::layout::front(room.slots[0]));
        let t = text(&here_lines(&st));
        assert_eq!(t[0], " ¤ Credchip · ¤340", "{t:?}");
        assert_eq!(t[1], "   summon · space");
        put(&mut st, crate::rigroom::layout::front(room.slots[1]));
        assert_eq!(text(&here_lines(&st)), [" · free slot"]);
        put(&mut st, room.doors[0].offset(1, 0));
        let t = text(&here_lines(&st));
        let name = crate::net::building_name(&st, kiroflux);
        assert!(t[0].starts_with(&format!(" ▯ {name} · net · ")), "{t:?}");
        assert_eq!(t[1], "   step through to its gate");
        put(&mut st, room.doors[3].offset(1, 0));
        assert_eq!(text(&here_lines(&st)), [" ▯ free line"]);
        put(&mut st, room.trunk.offset(0, -1));
        assert_eq!(text(&here_lines(&st)), [" ▸ the grid · your block"]);
    }

    #[test]
    fn the_fight_reads_under_you() {
        use crate::rigroom::mastiff;
        use crate::rigroom::testutil::{BLACK_SLOT, a_black_chip};
        let mut st = a_black_chip();
        mastiff::wake(&mut st, BLACK_SLOT).unwrap();
        let m = room(&st).unwrap().mastiff.clone().unwrap();
        let west = citygen::Pos::new(m.pos.x - 3, m.pos.y + 1);
        for _ in 0..3 {
            mastiff::hit(&mut st, west);
        }
        assert_eq!(
            text(&here_lines(&st)),
            [
                " ◆ MASTIFF · black ICE",
                "   COAT  ▮▮▮▮▮▯▯▯ 5/8",
                "   CORE  sealed under the coat",
                "   LEASH the red ground",
            ]
        );
        mastiff::turn(&mut st);
        let t = text(&here_lines(&st));
        assert_eq!(t[3], "   BITE  next turn · the × tiles", "{t:?}");
        assert_eq!(t[4], "   LEASH the red ground");
        // The coat gone, the core's hits.
        for _ in 0..5 {
            mastiff::hit(&mut st, west);
        }
        let t = text(&here_lines(&st));
        assert_eq!(t[1], "   COAT  ▯▯▯▯▯▯▯▯ 0/8");
        assert_eq!(t[2], "   CORE  2 hits");
        // The reader's row and the seats say the fight.
        let p = text(&panel_lines(&st));
        let black = (p.iter())
            .find(|l| l.contains("Black credchip"))
            .expect("the black chip's row");
        assert!(black.trim_end().ends_with("awake"), "{p:#?}");
        assert!(p.iter().any(|l| l.contains("SEATS 2/2")), "{p:#?}");
        assert!(p.iter().any(|l| l.contains("◉ attack · MASTIFF")), "{p:#?}");
    }

    #[test]
    fn off_the_leash_the_seats_read_idle() {
        use crate::rigroom::mastiff;
        use crate::rigroom::testutil::{BLACK_SLOT, a_black_chip};
        let mut st = a_black_chip();
        mastiff::wake(&mut st, BLACK_SLOT).unwrap();
        let seat_rows = |st: &AppState| -> Vec<String> {
            (text(&panel_lines(st)).into_iter())
                .filter(|l| l.contains("◉ attack · MASTIFF"))
                .collect()
        };
        let box_lines = |st: &AppState| -> Vec<String> {
            let v = crate::rigroom::view(st).unwrap();
            (v.seats.iter().flatten())
                .filter(|s| s.on == "MASTIFF")
                .map(|s| s.state.clone())
                .collect()
        };
        // On the leash: the seat's chance, as its beam rolls it.
        let odds = mastiff::seat_chance(&st, 0).expect("a shot");
        assert_eq!(box_lines(&st)[0], format!("seat 1 · {odds}%"));
        assert!(seat_rows(&st)[0].trim_end().ends_with(&format!("{odds}%")));
        // Off it the seats sit out, and say so.
        let r = room(&st).unwrap().clone();
        let leash = r.mastiff.as_ref().unwrap().leash;
        let away = (r.hall.iter())
            .find(|p| !leash.contains(*p) && r.construct.tiles[*p].is_walkable())
            .unwrap();
        st.player.as_mut().unwrap().pos = away;
        assert!(mastiff::seat_chance(&st, 0).is_err());
        assert_eq!(box_lines(&st), ["seat 1 · idle", "seat 2 · idle"]);
        let rows = seat_rows(&st);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter().all(|l| l.trim_end().ends_with("idle")),
            "{rows:#?}"
        );
    }

    #[test]
    fn a_bite_shows_on_the_body_line() {
        let (mut st, _) = the_mocks_rig();
        let t = text(&panel_lines(&st));
        assert!(t.last().unwrap().ends_with("BODY at home"), "{t:#?}");
        let max = st.character.hp_max(&st.content);
        st.character.hp = max - 14;
        let t = text(&panel_lines(&st));
        let want = format!("BODY at home · HP {}/{max}", max - 14);
        assert!(t.last().unwrap().contains(&want), "{t:#?}");
    }

    #[test]
    fn a_sites_backdoor_reads_site_in_the_lines_and_under_you() {
        let (mut st, _) = the_mocks_rig();
        let city = st.city.as_ref().unwrap();
        let home = st.home.unwrap().home.building;
        let site = (city.buildings.iter().map(|b| b.id))
            .find(|b| {
                *b != home
                    && crate::sitenet::is_site(&st, *b)
                    && crate::constructs::city_gate(city, *b).is_some()
                    && st.backdoors.live(*b).is_none()
            })
            .expect("a site with a gate");
        let seed = city.seed;
        let now = st.net.now(&st);
        st.backdoors.plant_net(site, 1, seed, now, 4);
        crate::rigroom::refresh(&mut st);
        let t = text(&panel_lines(&st));
        let name = crate::net::building_name(&st, site);
        let row = |t: &[String]| {
            t.iter()
                .find(|l| l.contains(&name) && l.contains("site"))
                .cloned()
        };
        let r = row(&t).unwrap_or_default();
        assert!(r.contains("site · 4d") && r.contains("▯"), "{t:#?}");
        // At its door.
        let room = room(&st).unwrap().clone();
        let at = crate::rigroom::door_lines(&st)
            .iter()
            .position(|b| *b == Some(site))
            .unwrap();
        st.player.as_mut().unwrap().pos = room.doors[at].offset(1, 0);
        let h = text(&here_lines(&st));
        assert!(h[0].contains(&format!("{name} · site · 4d")), "{h:?}");
        // Held by a keepalive.
        crate::rig::jobs::start(&mut st, crate::rig::jobs::JobKind::Keepalive(site))
            .or_else(|_| {
                st.backdoors.jobs.running.pop();
                crate::rig::jobs::start(&mut st, crate::rig::jobs::JobKind::Keepalive(site))
            })
            .expect("keepalive");
        let t = text(&panel_lines(&st));
        let r = row(&t).unwrap_or_default();
        assert!(r.contains("site · held"), "{t:#?}");
    }
}
