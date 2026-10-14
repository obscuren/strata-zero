//! The play HUD's rows: status, location, bars, tabs, the footer zones and the rules.

use crate::anim::effects::pulse;
use crate::render::overlay::Digital;
use crate::ui::chrome::{CYAN, DIM, FAINT, MAGENTA, RULE, TEXT};
use crate::ui::sidebar::{Section, Sidebar};
use citygen::{City, Pos, StratumKind, SubnetOwner};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

pub const SLOTS: usize = 5;
/// Ticks a fired slot stays inverted.
pub const FLASH_TICKS: u64 = 6;
pub const AMBER: Color = Color::Rgb(255, 200, 80);
pub const GREEN: Color = Color::Rgb(80, 230, 120);
pub const DIM_AMBER: Color = Color::Rgb(190, 150, 60);
pub const RULE_STYLE: Style = Style::new().fg(RULE);
const BADGE_FG: Color = Color::Rgb(10, 11, 16);
const TICKS_PER_MINUTE: u64 = 90;
const START_MINUTE: u64 = 22 * 60;

/// The clock at `tick`: 22:00 at tick 0, one minute per ninety ticks, wrapping at midnight.
pub fn clock_at(tick: u64) -> (u8, u8) {
    let m = (START_MINUTE + tick / TICKS_PER_MINUTE) % (24 * 60);
    ((m / 60) as u8, (m % 60) as u8)
}

/// A moon from 18:00 to 05:59, a sun otherwise.
pub fn clock_glyph(hour: u8) -> char {
    if hour >= 18 || hour <= 5 {
        '☾'
    } else {
        '☀'
    }
}

fn cell_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// The first `max` cells of `s`, never splitting a wide glyph.
fn take_cells(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = cell_width(c);
        if used + w > max {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}

/// Cuts `s` to `max` cells, ending in `…` when it was longer.
pub fn cut(s: &str, max: usize) -> String {
    if s.chars().map(cell_width).sum::<usize>() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = take_cells(s, max - 1);
    out.push('…');
    out
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(DIM))
}

fn faint(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(FAINT))
}

fn text(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::default().fg(TEXT))
}

fn cyan(s: impl Into<String>) -> Span<'static> {
    Span::styled(
        s.into(),
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    )
}

fn width_of(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|s| s.width()).sum()
}

/// Cuts a span list to `max` cells, ending in `…` when it was longer.
pub fn cut_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    if width_of(&spans) <= max {
        return spans;
    }
    if max == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut left = max - 1;
    for sp in spans {
        let w = sp.width();
        if w <= left {
            left -= w;
            out.push(sp);
            continue;
        }
        let kept = take_cells(&sp.content, left);
        if !kept.is_empty() {
            out.push(Span::styled(kept, sp.style));
        }
        out.push(Span::styled("…", sp.style));
        return out;
    }
    out
}

/// Pads a span list with spaces to exactly `width` cells.
fn padded(mut spans: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let w = width_of(&spans);
    if w < width {
        spans.push(Span::raw(" ".repeat(width - w)));
    }
    Line::from(spans)
}

pub struct Status<'a> {
    pub dev: bool,
    pub handle: &'a str,
    pub level: u8,
    pub credits: u32,
    pub seed: u64,
    pub fps: Option<f32>,
}

/// ` DEV  VEX :: LVL 2 :: ¤ 320 [:: seed N :: F fps]`, cut to `width`.
pub fn status_line(s: &Status, width: u16) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    if s.dev {
        spans.push(Span::styled(
            " DEV ",
            Style::default()
                .fg(BADGE_FG)
                .bg(AMBER)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    spans.push(cyan(s.handle.to_uppercase()));
    spans.push(dim(" :: LVL "));
    spans.push(text(s.level.to_string()));
    spans.push(dim(" :: ¤ "));
    spans.push(text(s.credits.to_string()));
    if s.dev {
        if let Some(fps) = s.fps {
            spans.push(dim(format!(" :: {fps:.0} fps")));
        }
        spans.push(dim(format!(" :: seed {}", s.seed)));
    }
    padded(cut_spans(spans, width as usize), width as usize)
}

/// The location on the left, the clock with its glyph at the right edge.
pub fn location_line(location: &str, clock: (u8, u8), width: u16) -> Line<'static> {
    let clock_text = format!("{} {:02}:{:02}", clock_glyph(clock.0), clock.0, clock.1);
    let room = (width as usize).saturating_sub(clock_text.chars().count() + 2);
    let loc = cut(location, room);
    let gap = (width as usize).saturating_sub(1 + loc.chars().count() + clock_text.chars().count());
    Line::from(vec![
        Span::raw(" "),
        cyan(loc),
        Span::raw(" ".repeat(gap)),
        text(clock_text),
    ])
}

/// Filled cells for `cur` of `max` out of `cells`.
pub fn bar_cells_for(cur: u32, max: u32, cells: usize) -> usize {
    if max == 0 {
        return 0;
    }
    ((cells as f64 * cur.min(max) as f64 / max as f64).round() as usize).min(cells)
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

fn bar_spans(filled: usize, cells: usize, colour: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled("█".repeat(filled), Style::default().fg(colour)),
        Span::styled(
            "░".repeat(cells.saturating_sub(filled)),
            Style::default().fg(colour).add_modifier(Modifier::DIM),
        ),
    ]
}

/// ` {label:<5} {cur}/{max} ` then the bar to the column's edge but one.
pub fn bar_row(label: &str, cur: u32, max: u32, colour: Color, width: u16) -> Line<'static> {
    let cells = (width as usize).saturating_sub(16);
    let mut spans = vec![
        dim(format!(" {label:<5} ")),
        text(format!("{:>7} ", format!("{cur}/{max}"))),
    ];
    spans.extend(bar_spans(bar_cells_for(cur, max, cells), cells, colour));
    padded(spans, width as usize)
}

/// ` CARRY w/cap kg ` then the bar; magenta when over capacity.
pub fn carry_row(weight: f32, cap: f32, width: u16) -> Line<'static> {
    let head = format!(" {weight:.1}/{cap:.1} kg ");
    let cells = (width as usize).saturating_sub(6 + head.chars().count() + 1);
    let colour = if weight > cap { MAGENTA } else { CYAN };
    let filled = if cap <= 0.0 {
        0
    } else {
        ((cells as f32 * (weight / cap).min(1.0)).round() as usize).min(cells)
    };
    let mut spans = vec![dim(" CARRY"), text(head)];
    spans.extend(bar_spans(filled, cells, colour));
    padded(spans, width as usize)
}

fn tab(key: &str, label: &str, on: bool, shown: bool) -> Vec<Span<'static>> {
    let word = if on {
        format!("{label}*")
    } else {
        label.to_string()
    };
    let style = if on && shown {
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    };
    vec![
        faint("["),
        Span::styled(key.to_string(), Style::default().fg(CYAN)),
        faint("]"),
        Span::styled(word, style),
    ]
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// (dev row, main row); the dev row is blank outside dev mode. A tab that is on but
/// whose pane got no rows (`drawn` lacks it) keeps its star and dims.
pub fn tab_rows(flags: &Sidebar, dev: bool, drawn: &[Section]) -> (Line<'static>, Line<'static>) {
    let row = |sections: &[Section], first: usize| {
        let mut spans = vec![Span::raw(" ")];
        for (i, s) in sections.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.extend(tab(
                &format!("F{}", first + i),
                &capitalised(s.label()),
                flags.is_on(*s),
                drawn.contains(s),
            ));
        }
        Line::from(spans)
    };
    let main = row(
        &[Section::Here, Section::Quest, Section::Map, Section::Log],
        1,
    );
    let dev_row = if dev {
        row(&[Section::Inspect, Section::Legend], 5)
    } else {
        Line::from("")
    };
    (dev_row, main)
}

pub const WALK_VERBS: &[(&str, &str)] = &[
    ("space", "act"),
    ("enter", "jack"),
    ("v", "iew"),
    ("m", "sgs"),
    ("?", "keys"),
];
pub const EXPLORE_VERBS: &[(&str, &str)] = &[
    ("w", "alk"),
    ("r", "egen"),
    ("g", "configure"),
    ("v", "iew"),
    ("m", "sgs"),
    ("?", "keys"),
];

fn verb_spans(verbs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, (k, w)) in verbs.iter().enumerate() {
        if i > 0 {
            out.push(Span::raw(" "));
        }
        out.push(faint("["));
        out.push(Span::styled(k.to_string(), Style::default().fg(CYAN)));
        out.push(faint("]"));
        out.push(dim(w.to_string()));
    }
    out
}

/// ` STATUS: …` at the left, `HERE: …` at 40% of the width, the verbs right-aligned;
/// zones are cut so they never touch.
pub fn footer_status(
    status: Vec<Span<'static>>,
    here: &str,
    verbs: &[(&str, &str)],
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let verbs = verb_spans(verbs);
    let verbs_x = w.saturating_sub(width_of(&verbs) + 1);
    // HERE starts at 40% of the width, or earlier when the verbs would leave it under
    // twenty cells.
    let here_x = (w * 40 / 100).min(verbs_x.saturating_sub(21));
    let mut zone1 = vec![dim(" STATUS: ")];
    zone1.extend(status);
    let mut spans = cut_spans(zone1, here_x.saturating_sub(1));
    let zone2 = cut_spans(
        vec![dim("HERE: "), text(here.to_string())],
        verbs_x.saturating_sub(here_x + 1),
    );
    let w1 = width_of(&spans);
    spans.push(Span::raw(" ".repeat(here_x.saturating_sub(w1))));
    spans.extend(zone2);
    let w2 = width_of(&spans);
    spans.push(Span::raw(" ".repeat(verbs_x.saturating_sub(w2))));
    spans.extend(verbs);
    padded(spans, w)
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
    /// Fired a moment ago: the slot draws inverted.
    pub flash: bool,
}

/// ` ABILITIES: ` then the five slots, then the deck (and console) hints at the right.
pub fn footer_abilities(slots: &[Slot], dev: bool, width: u16) -> Line<'static> {
    let mut spans = vec![dim(" ABILITIES: ")];
    for i in 0..SLOTS {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        match slots.get(i) {
            Some(slot) => {
                let (num, name) = if slot.flash {
                    let hot = Style::default()
                        .fg(BADGE_FG)
                        .bg(CYAN)
                        .add_modifier(Modifier::BOLD);
                    (
                        Span::styled(format!("{} ", i + 1), hot),
                        Span::styled(cut(&slot.name.to_lowercase(), 10), hot),
                    )
                } else {
                    (
                        cyan(format!("{} ", i + 1)),
                        text(cut(&slot.name.to_lowercase(), 10)),
                    )
                };
                spans.push(num);
                spans.push(name);
                match slot.state {
                    SlotState::Ready => spans.push(Span::styled(" ●", Style::default().fg(CYAN))),
                    SlotState::Cooling(secs) => spans.push(dim(format!(" {secs}s"))),
                }
            }
            None => spans.push(dim(format!("{} ──", i + 1))),
        }
    }
    let right = if dev {
        verb_spans(&[(":", "console"), ("tab", "deck")])
    } else {
        verb_spans(&[("tab", "deck")])
    };
    let used = width_of(&spans);
    let gap = (width as usize).saturating_sub(used + width_of(&right) + 1);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    padded(spans, width as usize)
}

/// A heavy rule with `glyph` where the vertical rule crosses it.
pub fn rule_row(width: u16, cross: u16, glyph: char) -> Line<'static> {
    let s: String = (0..width)
        .map(|x| if x == cross { glyph } else { '━' })
        .collect();
    Line::from(Span::styled(s, RULE_STYLE))
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
    use crate::ui::sidebar::{Section, Sidebar};
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

    fn status() -> Status<'static> {
        Status {
            dev: false,
            handle: "Vex",
            level: 2,
            credits: 320,
            seed: 7,
            fps: Some(30.0),
        }
    }

    #[test]
    fn clock_glyph_and_clock_at() {
        assert_eq!(clock_at(0), (22, 0));
        assert_eq!(clock_at(90 * 120), (0, 0));
        assert_eq!(clock_glyph(17), '☀');
        assert_eq!(clock_glyph(18), '☾');
        assert_eq!(clock_glyph(5), '☾');
        assert_eq!(clock_glyph(6), '☀');
    }

    #[test]
    fn status_line_has_handle_level_credits_and_dev_readouts() {
        let s = status_line(&status(), 79).to_string();
        assert_eq!(s.trim_end(), " VEX :: LVL 2 :: ¤ 320", "{s:?}");
        assert_eq!(s.chars().count(), 79);
        let d = status_line(
            &Status {
                dev: true,
                ..status()
            },
            79,
        )
        .to_string();
        assert!(d.starts_with("  DEV  "), "{d:?}");
        assert!(
            d.contains("VEX :: LVL 2 :: ¤ 320 :: 30 fps :: seed 7"),
            "{d:?}"
        );
        let no_fps = status_line(
            &Status {
                dev: true,
                fps: None,
                ..status()
            },
            79,
        )
        .to_string();
        assert!(
            no_fps.contains("seed 7") && !no_fps.contains("fps"),
            "{no_fps:?}"
        );
    }

    #[test]
    fn status_line_cuts_to_width() {
        let long = "A".repeat(80);
        let s = status_line(
            &Status {
                dev: true,
                handle: &long,
                ..status()
            },
            59,
        )
        .to_string();
        assert!(s.chars().count() <= 59 && s.contains('…'), "{s:?}");
    }

    #[test]
    fn location_line_keeps_the_clock() {
        let s = location_line("Night Market · Street", (22, 41), 40).to_string();
        assert_eq!(s.chars().count(), 40, "{s:?}");
        assert!(
            s.starts_with(" Night Market · Street") && s.ends_with("☾ 22:41"),
            "{s:?}"
        );
        let long = location_line(&"B".repeat(60), (9, 5), 36).to_string();
        assert_eq!(long.chars().count(), 36, "{long:?}");
        assert!(long.ends_with("… ☀ 09:05"), "{long:?}");
    }

    #[test]
    fn bar_rows_fill_the_column() {
        let s = bar_row("HP", 38, 45, GREEN, 79).to_string();
        assert_eq!(s.chars().count(), 79, "{s:?}");
        assert!(s.starts_with(" HP      38/45 "), "{s:?}");
        let cells = 79 - 16;
        assert_eq!(s.matches('█').count() + s.matches('░').count(), cells);
        assert_eq!(s.matches('█').count(), bar_cells_for(38, 45, cells));
        let x = bar_row("XP", 240, 600, AMBER, 59).to_string();
        assert!(x.starts_with(" XP    240/600 "), "{x:?}");
        let z = bar_row("FOCUS", 0, 0, CYAN, 59).to_string();
        assert_eq!(z.matches('█').count(), 0);
        let c = carry_row(11.6, 16.0, 40).to_string();
        assert_eq!(c.chars().count(), 40, "{c:?}");
        assert!(
            c.starts_with(" CARRY 11.6/16.0 kg ") && c.contains('█'),
            "{c:?}"
        );
        let over = carry_row(20.0, 16.0, 40);
        assert!(over.spans.iter().any(|sp| sp.style.fg == Some(MAGENTA)));
        assert_eq!(bar_cells_for(1, 0, 10), 0);
        assert_eq!(bar_cells_for(99, 45, 10), 10);
    }

    #[test]
    fn tab_rows_fit_the_narrow_column() {
        let mut flags = Sidebar::default();
        flags.set(Section::Quest, false);
        let (dev, main) = tab_rows(&flags, true, &Section::ALL);
        let m = main.to_string();
        assert_eq!(
            m.trim_end(),
            " [F1]Here* [F2]Quest [F3]Map* [F4]Log*",
            "{m:?}"
        );
        assert_eq!(m.chars().count(), 38);
        assert_eq!(dev.to_string().trim_end(), " [F5]Inspect [F6]Legend");
        let (dev, _) = tab_rows(&flags, false, &Section::ALL);
        assert_eq!(dev.to_string().trim(), "");
    }

    #[test]
    fn footer_rows_have_three_zones_and_slots() {
        let status = vec![Span::raw("on foot · Street")];
        let s = footer_status(status, "3 items · terminal", WALK_VERBS, 120).to_string();
        assert_eq!(s.chars().count(), 120, "{s:?}");
        assert!(s.starts_with(" STATUS: on foot · Street"), "{s:?}");
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars[48..54].iter().collect::<String>(), "HERE: ", "{s:?}");
        assert!(
            s.trim_end()
                .ends_with("[space]act [enter]jack [v]iew [m]sgs [?]keys"),
            "{s:?}"
        );
        let narrow = footer_status(
            vec![Span::raw("x".repeat(60))],
            &"y".repeat(60),
            EXPLORE_VERBS,
            100,
        )
        .to_string();
        assert_eq!(narrow.chars().count(), 100, "{narrow:?}");
        assert!(
            narrow.trim_end().ends_with("[?]keys") && narrow.matches('…').count() == 2,
            "{narrow:?}"
        );
        // At 100 columns the HERE zone keeps at least twenty cells even with the long explore verbs.
        assert!(narrow.contains("HERE: yyyyyyyyyyyyy"), "{narrow:?}");
        let slots = vec![
            Slot {
                name: "Netscan".into(),
                state: SlotState::Ready,
                flash: false,
            },
            Slot {
                name: "Ping".into(),
                state: SlotState::Cooling(12),
                flash: false,
            },
        ];
        let a = footer_abilities(&slots, true, 120).to_string();
        assert!(
            a.starts_with(" ABILITIES: 1 netscan ●   2 ping 12s   3 ──   4 ──   5 ──"),
            "{a:?}"
        );
        assert!(a.trim_end().ends_with("[:]console [tab]deck"), "{a:?}");
        let a = footer_abilities(&slots, false, 100).to_string();
        assert!(
            a.trim_end().ends_with("[tab]deck") && !a.contains("console"),
            "{a:?}"
        );
    }

    #[test]
    fn rule_row_crosses_at_the_column() {
        let r = rule_row(100, 63, '┳').to_string();
        assert_eq!(r.chars().count(), 100);
        assert_eq!(r.chars().nth(63), Some('┳'));
        assert_eq!(r.matches('━').count(), 99);
    }

    #[test]
    fn location_names_building_district_and_net() {
        let c = city();
        let b = c.buildings.iter().find(|b| b.name.is_some()).unwrap();
        assert_eq!(
            location(&c, StratumKind::Street, b.footprint.center()),
            format!("{} · Street", b.name.clone().unwrap())
        );
        assert_eq!(
            location(&c, StratumKind::Street, Pos::new(-5, -5)),
            "the outskirts · Street"
        );
        assert_eq!(
            location(&c, StratumKind::Net, Pos::new(-5, -5)),
            format!("{} · Net", c.name)
        );
    }

    #[test]
    fn cut_counts_cells_not_chars() {
        assert_eq!(cut("⚡⚡⚡", 4), "⚡…");
        assert_eq!(cut("⚡⚡", 4), "⚡⚡");
        assert_eq!(cut("abc", 0), "");
        let spans = cut_spans(vec![Span::raw("⚡⚡⚡")], 4);
        assert_eq!(width_of(&spans), 3, "{spans:?}");
        assert!(cut_spans(vec![Span::raw("abcd")], 0).is_empty());
        assert_eq!(width_of(&cut_spans(vec![Span::raw("abcd")], 1)), 1);
    }

    #[test]
    fn status_line_keeps_fps_before_the_seed() {
        let s = status_line(
            &Status {
                dev: true,
                seed: 7_167_344_703_327_711_647,
                fps: Some(27.0),
                ..status()
            },
            59,
        )
        .to_string();
        assert!(s.contains(":: 27 fps :: seed 71673"), "{s:?}");
        assert_eq!(s.chars().count(), 59);
    }

    #[test]
    fn brackets_are_faint_and_words_dim() {
        let (_, main) = tab_rows(&Sidebar::default(), false, &Section::ALL);
        let open = main.spans.iter().find(|sp| sp.content == "[").unwrap();
        assert_eq!(open.style.fg, Some(FAINT));
        let key = main.spans.iter().find(|sp| sp.content == "F1").unwrap();
        assert_eq!(key.style.fg, Some(CYAN));
        let s = footer_status(vec![], "nothing", WALK_VERBS, 100);
        let word = s.spans.iter().find(|sp| sp.content == "act").unwrap();
        assert_eq!(word.style.fg, Some(DIM));
        let bracket = s.spans.iter().find(|sp| sp.content == "]").unwrap();
        assert_eq!(bracket.style.fg, Some(FAINT));
    }

    #[test]
    fn fired_slot_draws_inverted() {
        let slots = vec![Slot {
            name: "Netscan".into(),
            state: SlotState::Cooling(10),
            flash: true,
        }];
        let line = footer_abilities(&slots, false, 100);
        let name = line
            .spans
            .iter()
            .find(|sp| sp.content == "netscan")
            .unwrap();
        assert_eq!(name.style.bg, Some(CYAN), "{line:?}");
        assert_eq!(name.style.fg, Some(Color::Rgb(10, 11, 16)));
        let calm = footer_abilities(
            &[Slot {
                name: "Netscan".into(),
                state: SlotState::Ready,
                flash: false,
            }],
            false,
            100,
        );
        let name = calm
            .spans
            .iter()
            .find(|sp| sp.content == "netscan")
            .unwrap();
        assert_eq!(name.style.bg, None);
    }

    #[test]
    fn squeezed_pane_tab_is_dim() {
        let flags = Sidebar::default();
        let (_, main) = tab_rows(
            &flags,
            false,
            &[Section::Here, Section::Quest, Section::Log],
        );
        let map = main.spans.iter().find(|sp| sp.content == "Map*").unwrap();
        assert_eq!(map.style.fg, Some(DIM), "{main:?}");
        let here = main.spans.iter().find(|sp| sp.content == "Here*").unwrap();
        assert_eq!(here.style.fg, Some(CYAN), "{main:?}");
    }
}
