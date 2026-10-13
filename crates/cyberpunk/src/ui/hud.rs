//! The play HUD's single rows: header, bars and the action bar.
#![allow(dead_code)] // wired into the map screen by the HUD frame

use crate::anim::effects::pulse;
use crate::render::overlay::Digital;
use crate::ui::chrome::{CYAN, DIM, TEXT};
use citygen::{City, Pos, StratumKind, SubnetOwner};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub const BAR_CELLS: usize = 12;
pub const SLOTS: usize = 5;
pub const AMBER: Color = Color::Rgb(255, 200, 80);
pub const GREEN: Color = Color::Rgb(80, 230, 120);
const DIM_AMBER: Color = Color::Rgb(190, 150, 60);
const BADGE_FG: Color = Color::Rgb(10, 11, 16);

/// Cuts `s` to `max` cells, ending in `…` when it was longer.
pub fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

pub struct Header<'a> {
    pub dev: bool,
    pub level: u8,
    pub credits: u32,
    pub location: &'a str,
    pub seed: u64,
    pub fps: Option<f32>,
    pub clock: (u8, u8),
}

/// Left: badge, level, credits. Centre: the location. Right: dev readouts and the clock.
pub fn header(h: &Header, width: u16) -> Line<'static> {
    let w = width as usize;
    let mut spans = vec![Span::raw(" ")];
    if h.dev {
        spans.push(Span::styled(
            " DEV ",
            Style::default()
                .fg(BADGE_FG)
                .bg(AMBER)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(
        format!("LVL {}   ¤ {}", h.level, h.credits),
        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
    ));
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let mut right = String::new();
    if h.dev {
        right.push_str(&format!("seed {} · ", h.seed));
        if let Some(fps) = h.fps {
            right.push_str(&format!("{fps:.0} fps · "));
        }
    }
    let clock = format!("{:02}:{:02}", h.clock.0, h.clock.1);
    let right_w = right.chars().count() + clock.chars().count() + 1;
    let room = w.saturating_sub(used + right_w);
    let centre = cut(h.location, room.saturating_sub(2));
    let cw = centre.chars().count();
    let ideal = w.saturating_sub(cw) / 2;
    let start = ideal.max(used + 1).min(used + room.saturating_sub(cw));
    spans.push(Span::raw(" ".repeat(start.saturating_sub(used))));
    spans.push(Span::styled(
        centre,
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(
        " ".repeat(w.saturating_sub(start + cw + right_w)),
    ));
    spans.push(Span::styled(right, Style::default().fg(DIM)));
    spans.push(Span::styled(
        clock,
        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(" "));
    Line::from(spans)
}

pub struct Bars {
    pub hp: (u32, u32),
    pub focus: (u32, u32),
    pub xp: (u32, u32),
    pub tick: u64,
}

/// Filled cells for `cur` of `max`.
pub fn bar_cells(cur: u32, max: u32) -> usize {
    if max == 0 {
        return 0;
    }
    ((BAR_CELLS as f64 * cur.min(max) as f64 / max as f64).round() as usize).min(BAR_CELLS)
}

/// Green above half, amber above a quarter, pulsing magenta at a quarter or below.
pub fn hp_colour(cur: u32, max: u32, tick: u64) -> Color {
    let frac = if max == 0 {
        0.0
    } else {
        cur as f32 / max as f32
    };
    if frac > 0.5 {
        GREEN
    } else if frac > 0.25 {
        AMBER
    } else {
        let f = 0.6 + 0.4 * pulse(tick, 20);
        Color::Rgb((255.0 * f) as u8, 0, (200.0 * f) as u8)
    }
}

fn bar(label: &str, cur: u32, max: u32, colour: Color) -> Vec<Span<'static>> {
    let filled = bar_cells(cur, max);
    vec![
        Span::styled(format!("{label} "), Style::default().fg(DIM)),
        Span::styled("█".repeat(filled), Style::default().fg(colour)),
        Span::styled(
            "░".repeat(BAR_CELLS - filled),
            Style::default().fg(colour).add_modifier(Modifier::DIM),
        ),
        Span::styled(format!(" {cur}/{max}"), Style::default().fg(TEXT)),
    ]
}

pub fn bars(b: &Bars) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(bar("HP", b.hp.0, b.hp.1, hp_colour(b.hp.0, b.hp.1, b.tick)));
    spans.push(Span::raw("    "));
    spans.extend(bar("FOCUS", b.focus.0, b.focus.1, CYAN));
    spans.push(Span::raw("    "));
    spans.extend(bar("XP", b.xp.0, b.xp.1, DIM_AMBER));
    Line::from(spans)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    Ready,
    /// Seconds left.
    Cooling(u64),
}

pub struct Slot {
    pub name: String,
    pub state: SlotState,
}

pub enum BarMode<'a> {
    Walk(&'a [Slot]),
    Explore,
}

/// Slots or the camera verbs on the left, the fixed hints on the right.
pub fn action_bar(mode: &BarMode, dev: bool, width: u16) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    match mode {
        BarMode::Walk(slots) => {
            for i in 0..SLOTS {
                if i > 0 {
                    spans.push(Span::raw("   "));
                }
                match slots.get(i) {
                    Some(slot) => {
                        spans.push(Span::styled(
                            format!("{} ", i + 1),
                            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            cut(&slot.name.to_lowercase(), 10),
                            Style::default().fg(TEXT),
                        ));
                        match slot.state {
                            SlotState::Ready => {
                                spans.push(Span::styled(" ●", Style::default().fg(CYAN)))
                            }
                            SlotState::Cooling(secs) => spans
                                .push(Span::styled(format!(" {secs}s"), Style::default().fg(DIM))),
                        }
                    }
                    None => spans.push(Span::styled(
                        format!("{} ──", i + 1),
                        Style::default().fg(DIM),
                    )),
                }
            }
        }
        BarMode::Explore => spans.push(Span::styled(
            "w walk   r regen   g configure",
            Style::default().fg(TEXT),
        )),
    }
    let right = if dev {
        ": console   tab deck   ? keys"
    } else {
        "tab deck   ? keys"
    };
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let gap = (width as usize).saturating_sub(used + right.chars().count() + 1);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(Span::styled(right.to_string(), Style::default().fg(DIM)));
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Where a tile is, for the header: `Kowloon Noodle · Street`, `Helix Biotek · Net`.
pub fn location(city: &City, stratum: StratumKind, pos: Pos) -> String {
    if stratum == StratumKind::Net {
        let owner = Digital::node_under(city, stratum, pos)
            .map(|id| city.net.node(id))
            .map(|n| match city.net.subnet(n.subnet).owner {
                SubnetOwner::Building(b) => {
                    let b = city.building(b);
                    b.name.clone().unwrap_or_else(|| b.kind.label().to_string())
                }
                SubnetOwner::District(d) => city.district(d).name.clone(),
                SubnetOwner::City => city.name.clone(),
            })
            .unwrap_or_else(|| city.name.clone());
        return format!("{owner} · Net");
    }
    let tile = city.tile(stratum, pos);
    let place = tile
        .and_then(|t| t.building)
        .and_then(|b| city.building_opt(b))
        .and_then(|b| b.name.clone())
        .or_else(|| {
            tile.and_then(|t| t.district)
                .and_then(|d| city.district_opt(d))
                .map(|d| d.name.clone())
        })
        .unwrap_or_else(|| "the outskirts".to_string());
    format!("{place} · {}", stratum.label())
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    fn base() -> Header<'static> {
        Header {
            dev: false,
            level: 3,
            credits: 320,
            location: "Night Market · Street",
            seed: 7,
            fps: Some(30.0),
            clock: (22, 41),
        }
    }

    #[test]
    fn header_has_three_zones() {
        let s = header(&base(), 100).to_string();
        assert_eq!(s.chars().count(), 100, "{s:?}");
        assert!(s.starts_with(" LVL 3   ¤ 320"), "{s:?}");
        assert!(s.trim_end().ends_with("22:41"), "{s:?}");
        assert!(!s.contains("DEV") && !s.contains("seed"));
        let at = s.find("Night Market").unwrap();
        assert!((35..=45).contains(&at), "{at}");
        let dev = header(
            &Header {
                dev: true,
                ..base()
            },
            100,
        )
        .to_string();
        assert!(
            dev.contains(" DEV ") && dev.contains("seed 7 · 30 fps · 22:41"),
            "{dev:?}"
        );
        let no_fps = header(
            &Header {
                dev: true,
                fps: None,
                ..base()
            },
            100,
        )
        .to_string();
        assert!(
            no_fps.contains("seed 7 · 22:41") && !no_fps.contains("fps"),
            "{no_fps:?}"
        );
    }

    #[test]
    fn header_keeps_the_clock_when_the_location_is_long() {
        let long = "A".repeat(120);
        let s = header(
            &Header {
                dev: true,
                location: &long,
                ..base()
            },
            100,
        )
        .to_string();
        assert_eq!(s.chars().count(), 100, "{s:?}");
        assert!(s.trim_end().ends_with("22:41") && s.contains('…'), "{s:?}");
        assert!(s.contains("seed 7"), "{s:?}");
    }

    #[test]
    fn bar_cells_round_and_handle_zero() {
        assert_eq!(bar_cells(0, 45), 0);
        assert_eq!(bar_cells(45, 45), 12);
        assert_eq!(bar_cells(22, 45), 6);
        assert_eq!(bar_cells(1, 0), 0);
        assert_eq!(bar_cells(99, 45), 12);
    }

    #[test]
    fn hp_colour_steps_at_half_and_quarter() {
        assert_eq!(hp_colour(23, 45, 0), GREEN);
        assert_eq!(hp_colour(22, 45, 0), AMBER);
        assert_eq!(hp_colour(12, 45, 0), AMBER);
        assert!(matches!(hp_colour(11, 45, 0), Color::Rgb(r, 0, _) if r > 100));
        assert!(matches!(hp_colour(0, 0, 0), Color::Rgb(_, 0, _)));
        let s = bars(&Bars {
            hp: (38, 45),
            focus: (12, 30),
            xp: (240, 600),
            tick: 0,
        })
        .to_string();
        assert!(
            s.contains("HP ██████████░░ 38/45")
                && s.contains("FOCUS █████░░░░░░░ 12/30")
                && s.contains("XP █████░░░░░░░ 240/600"),
            "{s:?}"
        );
    }

    #[test]
    fn action_bar_shows_slots_and_hints() {
        let slots = vec![
            Slot {
                name: "Netscan".into(),
                state: SlotState::Ready,
            },
            Slot {
                name: "Ping".into(),
                state: SlotState::Cooling(12),
            },
        ];
        let s = action_bar(&BarMode::Walk(&slots), false, 100).to_string();
        assert!(
            s.contains("1 netscan ●   2 ping 12s   3 ──   4 ──   5 ──"),
            "{s:?}"
        );
        assert!(
            s.trim_end().ends_with("tab deck   ? keys") && !s.contains("console"),
            "{s:?}"
        );
        assert_eq!(s.chars().count(), 100);
        let dev = action_bar(&BarMode::Walk(&slots), true, 100).to_string();
        assert!(
            dev.trim_end().ends_with(": console   tab deck   ? keys"),
            "{dev:?}"
        );
        let ex = action_bar(&BarMode::Explore, false, 100).to_string();
        assert!(ex.starts_with(" w walk   r regen   g configure"), "{ex:?}");
    }

    #[test]
    fn action_bar_cuts_long_names() {
        let slots = vec![Slot {
            name: "Electromagnetic pulse".into(),
            state: SlotState::Ready,
        }];
        let s = action_bar(&BarMode::Walk(&slots), false, 100).to_string();
        assert!(s.contains("1 electroma… ●"), "{s:?}");
        assert_eq!(s.chars().count(), 100);
        assert_eq!(cut("abc", 3), "abc");
        assert_eq!(cut("abcd", 3), "ab…");
        assert_eq!(cut("abcd", 0), "");
    }

    #[test]
    fn location_names_building_district_and_net() {
        let c = city();
        let b = c.buildings.iter().find(|b| b.name.is_some()).unwrap();
        let s = location(&c, StratumKind::Street, b.footprint.center());
        assert_eq!(s, format!("{} · Street", b.name.clone().unwrap()));
        let d = &c.districts[0];
        let street = (0..c.size.h)
            .flat_map(|y| (0..c.size.w).map(move |x| Pos::new(x as i32, y as i32)))
            .find(|p| {
                c.tile(StratumKind::Street, *p)
                    .is_some_and(|t| t.building.is_none() && t.district == Some(d.id))
            })
            .unwrap();
        assert_eq!(
            location(&c, StratumKind::Street, street),
            format!("{} · Street", d.name)
        );
        assert_eq!(
            location(&c, StratumKind::Street, Pos::new(-5, -5)),
            "the outskirts · Street"
        );
        let n = &c.net.nodes[0];
        let s = location(&c, StratumKind::Net, n.cyber);
        assert!(s.ends_with(" · Net") && s.len() > 6, "{s}");
        assert_eq!(
            location(&c, StratumKind::Net, Pos::new(-5, -5)),
            format!("{} · Net", c.name)
        );
    }
}
