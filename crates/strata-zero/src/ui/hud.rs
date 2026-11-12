//! The play HUD's rows: status, location, bars, tabs, the footer zones and the rules.

use crate::anim::effects::pulse;
use crate::interiors::Interiors;
use crate::ui::chrome::{CYAN, DIM, FAINT, MAGENTA, RULE, TEXT};
use crate::ui::sidebar::{Section, Sidebar};
use citygen::{City, FloorPlan, Place, Pos, StratumKind};
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
pub(crate) const BADGE_FG: Color = Color::Rgb(10, 11, 16);
const TICKS_PER_MINUTE: u64 = 90;
const START_MINUTE: u64 = 22 * 60;
/// The minute of the day the game starts at, for the simulation's clock.
pub const START_MINUTE_OF_DAY: u16 = 22 * 60;

/// The clock at `tick`: 22:00 at tick 0, one minute per ninety ticks, wrapping at midnight.
pub fn clock_at(tick: u64) -> (u8, u8) {
    let m = minute_at(tick) as u64;
    ((m / 60) as u8, (m % 60) as u8)
}

/// The minute of the day at `tick`, from the 22:00 start.
pub fn minute_at(tick: u64) -> u16 {
    ((START_MINUTE + tick / TICKS_PER_MINUTE) % (24 * 60)) as u16
}

/// The day at `tick`, counting from day 1 at tick 0.
pub fn day_at(tick: u64) -> u32 {
    ((START_MINUTE + tick / TICKS_PER_MINUTE) / (24 * 60)) as u32 + 1
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
    /// Unread mails, marked after the credits; none marks nothing.
    pub mail: usize,
}

/// ` DEV  VEX :: LVL 2 :: ¤ 320 [:: ▣ N NEW MAIL] [:: F fps :: seed N]`, cut to `width`.
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
    if s.mail > 0 {
        let words = match s.mail {
            1 => "▣ NEW MAIL".to_string(),
            n => format!("▣ {n} NEW MAIL"),
        };
        spans.push(dim(" :: "));
        spans.push(Span::styled(
            words,
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        ));
    }
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

/// How exposed you are, on its own band row: `dim · 2 eyes`, `bright · unseen`.
/// Empty off the strata.
pub fn exposure_row(exposure: &str, width: u16) -> Line<'static> {
    let body = cut(exposure, (width as usize).saturating_sub(1));
    padded(vec![Span::raw(" "), dim(body)], width as usize)
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

pub(crate) fn capitalised(s: &str) -> String {
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
        &[Section::Here, Section::Jobs, Section::Map, Section::Log],
        1,
    );
    let dev_row = if dev {
        row(&[Section::Inspect, Section::Legend], 5)
    } else {
        Line::from("")
    };
    (dev_row, main)
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

/// Everything the four-row footer reads. Strings come formatted; the footer only
/// places and cuts them.
pub struct Footer<'a> {
    /// `Night Market` on foot, `Hextek Holdings · Crown Circle` jacked in.
    pub place: String,
    /// The run was just saved: ` :: SAVED` after the place.
    pub saved: bool,
    pub jacked: bool,
    pub camera: bool,
    pub overburdened: bool,
    /// `TRACE ▰▰▱▱▱▱▱▱▱▱ 23 · ALERT 58 min`, jacked in.
    pub trace: Option<String>,
    pub hunted: bool,
    /// On foot on turns: `ENGAGED` at the right of row 1.
    pub engaged: bool,
    /// `TURN 3  YOU › grey ICE › black ICE`, engaged.
    pub turn: Option<String>,
    /// `RUN  crack · door lock  ▰▰▱ 2/3 · 40%`: in a fight, or calm when the run has
    /// no tether on the map.
    pub run: Option<String>,
    /// The ICE in sight, nearest first.
    pub ice: Vec<String>,
    pub slots_label: &'a str,
    pub slots: &'a [Slot],
    /// Not your turn: the keys draw hollow.
    pub hollow: bool,
    /// `(moves, moves max, ap, ap max)` while engaged.
    pub budgets: Option<(u8, u8, u8, u8)>,
    /// On your turn: `Some(false)` while you move, `Some(true)` while you act; the
    /// live pool's label lights.
    pub acting: Option<bool>,
    /// Row 2 while engaged: what is happening and the keys for it.
    pub prompt: Option<(String, String)>,
    /// On your act half: the fight's abilities and throwables, numbered. In a
    /// construct only the net-capable abilities, which follow the programs.
    pub act: Option<Vec<ActEntry>>,
    /// The pools draw lit: a boost just refilled them.
    pub flash_pools: bool,
}

/// One entry of the act half's `ACT` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActEntry {
    pub key: char,
    pub name: String,
    /// How many are left: set on a throwable.
    pub count: Option<u32>,
    pub state: ActState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActState {
    Ready,
    Cooling,
    /// Cannot be used right now.
    Refused,
}

/// The SAVED mark: its ` :: ` and the word.
const SAVED_FAINT: Color = Color::Rgb(100, 110, 125);
const SAVED_PICK: Color = Color::Rgb(0, 204, 184);
const ACT_KEY: Color = Color::Rgb(90, 90, 90);
const ACT_THROWN: Color = Color::Rgb(190, 190, 140);
const POOLS_FLASH_BG: Color = Color::Rgb(0, 90, 110);

fn entry_spans(e: &ActEntry) -> Vec<Span<'static>> {
    let name_fg = match (e.state, e.count) {
        (ActState::Ready, Some(_)) => ACT_THROWN,
        (ActState::Ready, None) => TEXT,
        _ => DIM,
    };
    let mut label = e.name.to_lowercase();
    if let Some(n) = e.count {
        label.push_str(&format!("×{n}"));
    }
    let mut spans = vec![
        Span::styled(format!("{} ", e.key), Style::default().fg(ACT_KEY)),
        Span::styled(label, Style::default().fg(name_fg)),
    ];
    if e.state == ActState::Cooling {
        spans.push(dim(" ◌"));
    }
    spans
}

/// The entries that fit in `avail` cells, whole ones only with a space between; a
/// ` …` ends the list when some were left out.
fn act_spans(entries: &[ActEntry], avail: usize) -> Vec<Span<'static>> {
    let cells: Vec<Vec<Span<'static>>> = entries.iter().map(entry_spans).collect();
    let total: usize =
        cells.iter().map(|c| width_of(c)).sum::<usize>() + cells.len().saturating_sub(1);
    let mut kept = Vec::new();
    let mut used = 0;
    let all = total <= avail;
    for c in cells {
        let w = width_of(&c) + usize::from(!kept.is_empty());
        if !all && used + w + 2 > avail {
            break;
        }
        used += w;
        kept.push(c);
    }
    let mut out = Vec::new();
    for (i, c) in kept.into_iter().enumerate() {
        if i > 0 {
            out.push(Span::raw(" "));
        }
        out.extend(c);
    }
    if !all && avail >= used + 2 {
        out.push(dim(" …"));
    }
    out
}

/// Row 4 on the act half: ` ACT  1 static 2 jolt …` with the pools on the right.
/// Whole entries drop off the end with `…` when they do not fit.
pub fn act_row(entries: &[ActEntry], right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let right = cut_spans(right, width / 2);
    let avail = width.saturating_sub(width_of(&right) + 1 + 6);
    let mut left = vec![dim(" ACT  ")];
    left.extend(act_spans(entries, avail));
    right_aligned(left, right, width)
}

fn right_aligned(left: Vec<Span<'static>>, right: Vec<Span<'static>>, w: usize) -> Line<'static> {
    // Neither side may eat the other: the right part keeps at most half the row.
    let right = cut_spans(right, w / 2);
    let rw = width_of(&right);
    let mut spans = cut_spans(left, w.saturating_sub(rw + 1));
    let used = width_of(&spans);
    spans.push(Span::raw(" ".repeat(w.saturating_sub(used + rw))));
    spans.extend(right);
    padded(cut_spans(spans, w), w)
}

/// The row under the top rule: the route of a rig run on the left, the trace walking
/// home on the right. An empty `route` leaves its side blank.
pub fn route_row(route: &str, trace: Option<&str>, width: u16) -> Line<'static> {
    let mut left = Vec::new();
    if !route.is_empty() {
        left.push(dim(" ROUTE  "));
        left.push(cyan(route.to_string()));
    }
    let right = match trace {
        Some(t) => vec![
            Span::styled(
                t.to_string(),
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ],
        None => Vec::new(),
    };
    right_aligned(left, right, width as usize)
}

/// The footer's four rows: place and trace with the one hint left; a blank; the
/// fight (turn, run, ICE); the slots with the budgets. Always four, so nothing
/// moves between the street, a calm construct and a fight.
pub fn footer_rows(f: &Footer, width: u16) -> [Line<'static>; 4] {
    let w = width as usize;
    // Row 1.
    let mut row1 = Vec::new();
    if f.jacked {
        row1.push(Span::styled(" ⚡ ", Style::default().fg(CYAN)));
        row1.push(text(f.place.clone()));
    } else if f.camera {
        row1.push(text(format!(" camera · {}", f.place)));
    } else {
        row1.push(text(format!(" on foot · {}", f.place)));
    }
    if f.saved {
        row1.push(Span::styled(" :: ", Style::default().fg(SAVED_FAINT)));
        row1.push(Span::styled(
            "SAVED",
            Style::default().fg(SAVED_PICK).add_modifier(Modifier::BOLD),
        ));
    }
    if f.overburdened {
        row1.push(Span::styled(
            " · overburdened",
            Style::default().fg(MAGENTA),
        ));
    }
    if let Some(trace) = &f.trace {
        let at = w * 40 / 100;
        let used = width_of(&row1);
        row1.push(Span::raw(" ".repeat(at.saturating_sub(used).max(2))));
        row1.push(Span::styled(trace.clone(), Style::default().fg(CYAN)));
    }
    let mut right1 = Vec::new();
    if f.hunted || f.engaged {
        right1.push(Span::styled(
            if f.hunted { "HUNTED" } else { "ENGAGED" },
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        ));
        right1.push(Span::raw("    "));
    }
    right1.push(faint("["));
    right1.push(cyan("?"));
    right1.push(faint("]"));
    let line1 = right_aligned(row1, right1, w);
    // Row 3: the fight.
    let mut row3 = Vec::new();
    if let Some(turn) = &f.turn {
        row3.push(Span::styled(
            format!(" {turn}"),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(run) = &f.run {
        row3.push(Span::raw(if row3.is_empty() { " " } else { "   " }));
        let (head, rest) = run.split_once(' ').unwrap_or((run.as_str(), ""));
        row3.push(Span::styled(
            head.to_string(),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
        row3.push(text(format!(" {rest}")));
    }
    let mut right3 = Vec::new();
    for (i, l) in f.ice.iter().enumerate() {
        if i > 0 {
            right3.push(Span::raw("   "));
        }
        let colour = if l.contains("black ICE") {
            Color::Rgb(255, 40, 40)
        } else if l.contains(" ICE") {
            MAGENTA
        } else {
            crate::render::people::FOE
        };
        right3.push(Span::styled(l.clone(), Style::default().fg(colour)));
    }
    let line3 = right_aligned(row3, right3, w);
    // Row 4: the slots and the budgets.
    let mut row4 = vec![dim(format!(" {:<9}  ", f.slots_label))];
    let programs = f.slots_label == "PROGRAMS";
    for i in 0..SLOTS {
        if i > 0 {
            row4.push(Span::raw("    "));
        }
        match f.slots.get(i) {
            Some(slot) => {
                let name = if programs {
                    cut(&slot.name.to_uppercase(), 10)
                } else {
                    cut(&slot.name.to_lowercase(), 10)
                };
                let (key, label) = if slot.flash {
                    let hot = Style::default()
                        .fg(BADGE_FG)
                        .bg(CYAN)
                        .add_modifier(Modifier::BOLD);
                    (
                        Span::styled(format!("[{}]", i + 1), hot),
                        Span::styled(format!(" {name}"), hot),
                    )
                } else if f.hollow {
                    (dim(format!("[{}]", i + 1)), dim(format!(" {name}")))
                } else {
                    (cyan(format!("[{}]", i + 1)), text(format!(" {name}")))
                };
                row4.push(key);
                row4.push(label);
                row4.push(match (slot.state, f.hollow) {
                    (_, true) => dim(" ○"),
                    (SlotState::Ready, false) => Span::styled(" ●", Style::default().fg(CYAN)),
                    (SlotState::Cooling(secs), false) => dim(format!(" {secs}s")),
                });
            }
            None => row4.push(dim(format!("[{}] ──", i + 1))),
        }
    }
    let mut right4 = Vec::new();
    if let Some((mv, mv_max, ap, ap_max)) = f.budgets {
        let badge = |label: &str, live: bool| {
            if live {
                Span::styled(
                    format!(" {label} "),
                    Style::default()
                        .fg(Color::Black)
                        .bg(CYAN)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                dim(format!(" {label} "))
            }
        };
        let moves = [
            badge("MOV", f.acting == Some(false)),
            Span::styled(
                format!(
                    "{}{}",
                    "▰".repeat(mv as usize),
                    "▱".repeat(mv_max.saturating_sub(mv) as usize)
                ),
                Style::default().fg(CYAN),
            ),
            text(format!(" {mv}/{mv_max}")),
        ];
        let points = [
            badge("AP", f.acting == Some(true)),
            Span::styled(
                format!(
                    "{}{}",
                    "●".repeat(ap as usize),
                    "○".repeat(ap_max.saturating_sub(ap) as usize)
                ),
                Style::default().fg(CYAN),
            ),
            text(format!(" {ap}/{ap_max}")),
        ];
        // The act half reads the action points first.
        let (first, second) = if f.act.is_some() {
            (points, moves)
        } else {
            (moves, points)
        };
        right4.extend(first);
        right4.push(Span::raw("   "));
        right4.extend(second);
    }
    if f.flash_pools {
        let lit = Style::default()
            .fg(Color::White)
            .bg(POOLS_FLASH_BG)
            .add_modifier(Modifier::BOLD);
        for sp in &mut right4 {
            sp.style = lit;
        }
    }
    let line4 = match &f.act {
        Some(entries) if !programs => act_row(entries, right4, w),
        Some(entries) => {
            // A construct: the programs keep their slots, the abilities follow.
            let rw = width_of(&cut_spans(right4.clone(), w / 2));
            let used = width_of(&row4) + 3;
            row4.push(Span::raw("   "));
            row4.extend(act_spans(entries, w.saturating_sub(rw + 1 + used)));
            right_aligned(row4, right4, w)
        }
        None => right_aligned(row4, right4, w),
    };
    // Row 2: what is happening on turns, and the keys for it.
    let row2 = match &f.prompt {
        Some((what, keys)) => vec![
            Span::styled(
                format!(" ▸ {what}"),
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ),
            dim(format!("   {keys}")),
        ],
        None => Vec::new(),
    };
    [line1, padded(row2, w), line3, line4]
}

/// A heavy rule with `glyph` where the vertical rule crosses it.
pub fn rule_row(width: u16, cross: u16, glyph: char) -> Line<'static> {
    let s: String = (0..width)
        .map(|x| if x == cross { glyph } else { '━' })
        .collect();
    Line::from(Span::styled(s, RULE_STYLE))
}

/// Where a tile is, for the header: `Kowloon Noodle · Street`, `Helix Biotek · Net`.
pub fn location(city: &City, interiors: &Interiors, place: Place, pos: Pos) -> String {
    let stratum = match place {
        Place::Stratum(k) => k,
        Place::Floor { building, level } => {
            let name = city
                .building_opt(building)
                .map(|b| {
                    b.name
                        .clone()
                        .unwrap_or_else(|| citygen::kind_label(city, b.id).to_string())
                })
                .unwrap_or_else(|| "inside".to_string());
            let _ = interiors;
            return format!("{name} · {}", FloorPlan::label(level));
        }
        Place::Construct(building) | Place::SiteNet { building, .. } => {
            let name = city
                .building_opt(building)
                .map(|b| {
                    b.name
                        .clone()
                        .unwrap_or_else(|| citygen::kind_label(city, b.id).to_string())
                })
                .unwrap_or_else(|| "inside".to_string());
            return format!("{name} · construct");
        }
    };
    if stratum == StratumKind::Net {
        // The district the grid runs over, not the nearest node's owner.
        let district = city
            .tile(stratum, pos)
            .and_then(|t| t.district)
            .or_else(|| city.tile(StratumKind::Street, pos).and_then(|t| t.district))
            .and_then(|d| city.district_opt(d))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| city.name.clone());
        return format!("{district} · Net");
    }
    if city.is_lot() {
        return format!("{} · {}", city.name, stratum.label());
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
            mail: 0,
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
    fn status_line_marks_unread_mail() {
        let mark = |mail: usize, dev: bool, width: u16| {
            status_line(
                &Status {
                    dev,
                    mail,
                    seed: 7_167_344_703_327_711_647,
                    ..status()
                },
                width,
            )
        };
        let one = mark(1, false, 79);
        assert_eq!(
            one.to_string().trim_end(),
            " VEX :: LVL 2 :: ¤ 320 :: ▣ NEW MAIL"
        );
        let lit = one.spans.iter().find(|s| s.content.contains('▣')).unwrap();
        assert_eq!(lit.content, "▣ NEW MAIL");
        assert_eq!(
            lit.style,
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
        );
        let sep = one
            .spans
            .iter()
            .rev()
            .find(|s| s.content == " :: ")
            .unwrap();
        assert_eq!(sep.style.fg, Some(DIM));
        assert_eq!(
            mark(2, false, 79).to_string().trim_end(),
            " VEX :: LVL 2 :: ¤ 320 :: ▣ 2 NEW MAIL"
        );
        assert!(!mark(0, false, 79).to_string().contains("MAIL"));
        // The left band at 100×30, dev readouts and a long seed after it: the mark stays
        // whole, the line its width.
        let lw = crate::screens::explore::layout(ratatui::layout::Rect::new(0, 0, 100, 30), false)
            .band_l[0]
            .width;
        let dev = mark(2, true, lw).to_string();
        assert_eq!(dev.chars().count(), lw as usize, "{dev:?}");
        assert!(
            dev.contains("¤ 320 :: ▣ 2 NEW MAIL :: 30 fps :: ") && dev.ends_with('…'),
            "{dev:?}"
        );
        // Narrower than the mark: cut like the rest, never past the width.
        let tight = mark(2, false, 30).to_string();
        assert_eq!(tight.chars().count(), 30, "{tight:?}");
        assert!(tight.ends_with('…'), "{tight:?}");
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
    fn exposure_row_fills_its_width_and_cuts() {
        let s = exposure_row("dim · 2 eyes", 20).to_string();
        assert_eq!(s.chars().count(), 20, "{s:?}");
        assert!(s.starts_with(" dim · 2 eyes"), "{s:?}");
        let short = exposure_row("bright · unseen", 10).to_string();
        assert_eq!(short.chars().count(), 10, "{short:?}");
        assert!(short.ends_with("…"), "{short:?}");
        assert_eq!(exposure_row("", 8).to_string(), " ".repeat(8));
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
        flags.set(Section::Jobs, false);
        let (dev, main) = tab_rows(&flags, true, &Section::ALL);
        let m = main.to_string();
        assert_eq!(
            m.trim_end(),
            " [F1]Here* [F2]Jobs [F3]Map* [F4]Log*",
            "{m:?}"
        );
        assert_eq!(m.chars().count(), 37);
        assert_eq!(dev.to_string().trim_end(), " [F5]Inspect [F6]Legend");
        let (dev, _) = tab_rows(&flags, false, &Section::ALL);
        assert_eq!(dev.to_string().trim(), "");
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
            location(
                &c,
                &Interiors::default(),
                Place::Stratum(StratumKind::Street),
                b.footprint.center()
            ),
            format!("{} · Street", b.name.clone().unwrap())
        );
        assert_eq!(
            location(
                &c,
                &Interiors::default(),
                Place::Stratum(StratumKind::Street),
                Pos::new(-5, -5)
            ),
            "the outskirts · Street"
        );
        assert_eq!(
            location(
                &c,
                &Interiors::default(),
                Place::Stratum(StratumKind::Net),
                Pos::new(-5, -5)
            ),
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
    }

    #[test]
    fn the_route_row_carries_the_route_and_the_trace_home() {
        let flat = |l: Line| {
            l.spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        let r = flat(route_row("zenbo ▸ a", None, 60));
        assert!(r.starts_with(" ROUTE  zenbo ▸ a"), "{r:?}");
        let t = flat(route_row("", Some("TRACE ▸ a · 3 min"), 60));
        assert!(t.trim_end().ends_with("TRACE ▸ a · 3 min"), "{t:?}");
        assert!(!t.contains("ROUTE"));
        let both = flat(route_row("zenbo ▸ a", Some("TRACE ▸ a · 3 min"), 60));
        assert!(both.contains("ROUTE  zenbo ▸ a") && both.contains("TRACE ▸ a · 3 min"));
    }

    #[test]
    fn fired_slot_draws_inverted() {
        let slots = vec![Slot {
            name: "Netscan".into(),
            state: SlotState::Cooling(10),
            flash: true,
        }];
        let footer = |slots: &[Slot]| {
            let f = Footer {
                place: "Street".into(),
                saved: false,
                jacked: false,
                camera: false,
                overburdened: false,
                trace: None,
                hunted: false,
                engaged: false,
                turn: None,
                run: None,
                ice: Vec::new(),
                slots_label: "ABILITIES",
                slots,
                hollow: false,
                budgets: None,
                acting: None,
                prompt: None,
                act: None,
                flash_pools: false,
            };
            footer_rows(&f, 100)[3].clone()
        };
        let line = footer(&slots);
        let name = line
            .spans
            .iter()
            .find(|sp| sp.content == " netscan")
            .unwrap();
        assert_eq!(name.style.bg, Some(CYAN), "{line:?}");
        assert_eq!(name.style.fg, Some(Color::Rgb(10, 11, 16)));
        let calm = footer(&[Slot {
            name: "Netscan".into(),
            state: SlotState::Ready,
            flash: false,
        }]);
        let name = calm
            .spans
            .iter()
            .find(|sp| sp.content == " netscan")
            .unwrap();
        assert_eq!(name.style.bg, None);
    }

    #[test]
    fn squeezed_pane_tab_is_dim() {
        let flags = Sidebar::default();
        let (_, main) = tab_rows(&flags, false, &[Section::Here, Section::Jobs, Section::Log]);
        let map = main.spans.iter().find(|sp| sp.content == "Map*").unwrap();
        assert_eq!(map.style.fg, Some(DIM), "{main:?}");
        let here = main.spans.iter().find(|sp| sp.content == "Here*").unwrap();
        assert_eq!(here.style.fg, Some(CYAN), "{main:?}");
    }

    fn slots_for_footer() -> Vec<Slot> {
        vec![
            Slot {
                name: "Crack".into(),
                state: SlotState::Ready,
                flash: false,
            },
            Slot {
                name: "Copy".into(),
                state: SlotState::Ready,
                flash: false,
            },
        ]
    }

    #[test]
    fn the_footer_is_four_rows_in_every_state() {
        let on_foot = Footer {
            place: "Night Market".into(),
            saved: false,
            jacked: false,
            camera: false,
            overburdened: false,
            trace: None,
            hunted: false,
            engaged: false,
            turn: None,
            run: None,
            ice: Vec::new(),
            slots_label: "ABILITIES",
            slots: &[Slot {
                name: "Netscan".into(),
                state: SlotState::Ready,
                flash: false,
            }],
            hollow: false,
            budgets: None,
            acting: None,
            prompt: None,
            act: None,
            flash_pools: false,
        };
        let rows: Vec<String> = footer_rows(&on_foot, 120)
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert_eq!(rows.len(), 4);
        assert!(
            rows[0].starts_with(" on foot · Night Market"),
            "{:?}",
            rows[0]
        );
        assert!(rows[0].trim_end().ends_with("[?]"), "{:?}", rows[0]);
        assert!(!rows[0].contains("HERE"), "{:?}", rows[0]);
        assert_eq!(rows[1].trim(), "");
        assert_eq!(rows[2].trim(), "");
        assert!(
            rows[3].starts_with(" ABILITIES  [1] netscan ●    [2] ──"),
            "{:?}",
            rows[3]
        );
        for r in &rows {
            assert_eq!(unicode_width::UnicodeWidthStr::width(r.as_str()), 120);
        }
        let calm = Footer {
            place: "Hextek Holdings · Crown Circle".into(),
            saved: false,
            jacked: true,
            camera: false,
            overburdened: false,
            trace: Some("TRACE ▰▰▱▱▱▱▱▱▱▱ 23".into()),
            hunted: false,
            engaged: false,
            turn: None,
            run: Some("RUN  crack · door lock  ▰▰▱ 2/3 · 40%".into()),
            ice: vec!["▣ grey ICE · asleep · 6/6 · 4 tiles".into()],
            slots_label: "PROGRAMS",
            slots: &slots_for_footer(),
            hollow: false,
            budgets: None,
            acting: None,
            prompt: None,
            act: None,
            flash_pools: false,
        };
        let rows: Vec<String> = footer_rows(&calm, 130)
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert!(
            rows[0].starts_with(" ⚡ Hextek Holdings · Crown Circle"),
            "{:?}",
            rows[0]
        );
        assert!(rows[0].contains("TRACE ▰▰▱▱▱▱▱▱▱▱ 23"), "{:?}", rows[0]);
        assert_eq!(rows[1].trim(), "");
        assert!(
            rows[2].starts_with(" RUN  crack · door lock"),
            "{:?}",
            rows[2]
        );
        assert!(
            rows[2]
                .trim_end()
                .ends_with("▣ grey ICE · asleep · 6/6 · 4 tiles"),
            "{:?}",
            rows[2]
        );
        assert!(
            rows[3].starts_with(" PROGRAMS   [1] CRACK ●    [2] COPY ●    [3] ──"),
            "{:?}",
            rows[3]
        );
        let engaged = Footer {
            turn: Some("TURN 3  YOU › grey ICE › black ICE".into()),
            run: Some("RUN  crack · door lock  ▰▰▱ 2/3 · 40%".into()),
            hunted: true,
            trace: Some("TRACE ▰▰▰▱▱▱▱▱▱▱ 34 · ALERT 58 min".into()),
            ice: vec![
                "▣ grey ICE · hunting · 2 tiles".into(),
                "▣ black ICE · hunting · 5 tiles".into(),
                "▣ white ICE · asleep · 9 tiles".into(),
            ],
            budgets: Some((3, 4, 2, 2)),
            ..calm
        };
        let rows: Vec<String> = footer_rows(&engaged, 130)
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert!(
            rows[0].contains("HUNTED") && rows[0].trim_end().ends_with("[?]"),
            "{:?}",
            rows[0]
        );
        assert!(
            rows[2].starts_with(" TURN 3  YOU › grey ICE › black ICE"),
            "{:?}",
            rows[2]
        );
        assert!(rows[2].contains("RUN  crack"), "{:?}", rows[2]);
        assert!(
            rows[3].trim_end().ends_with("MOV ▰▰▰▱ 3/4    AP ●● 2/2"),
            "{:?}",
            rows[3]
        );
        // Narrow: every row is still one line of the width, cut, never wrapped.
        let rows: Vec<String> = footer_rows(&engaged, 100)
            .iter()
            .map(|l| l.to_string())
            .collect();
        for r in &rows {
            assert_eq!(
                unicode_width::UnicodeWidthStr::width(r.as_str()),
                100,
                "{r:?}"
            );
        }
        assert!(rows[2].starts_with(" TURN 3"), "{:?}", rows[2]);
        assert!(
            rows[3].contains("MOV") && rows[3].contains("AP"),
            "{:?}",
            rows[3]
        );
        // An ICE turn: the keys go hollow.
        let theirs = Footer {
            hollow: true,
            turn: Some("TURN 3  grey ICE moves 2 › black ICE › YOU".into()),
            ..engaged
        };
        let rows: Vec<String> = footer_rows(&theirs, 130)
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert!(
            rows[3].contains("[1] CRACK ○    [2] COPY ○"),
            "{:?}",
            rows[3]
        );
    }

    fn act_entries() -> Vec<ActEntry> {
        let e = |key, name: &str, count| ActEntry {
            key,
            name: name.into(),
            count,
            state: ActState::Ready,
        };
        vec![
            e('1', "Static", None),
            e('2', "Jolt", None),
            e('3', "Shove", None),
            e('4', "Redline", None),
            e('5', "Deadeye", None),
            e('6', "Frag", Some(3)),
            e('7', "Flashbang", Some(2)),
            e('8', "EMP Puck", Some(2)),
            e('9', "Satchel Charge", Some(1)),
        ]
    }

    fn pools() -> Vec<Span<'static>> {
        vec![Span::raw("AP ●● 2/2  MOV 4/4")]
    }

    #[test]
    fn the_act_row_numbers_abilities_then_throwables_and_cuts_with_an_ellipsis() {
        let full = act_row(&act_entries(), pools(), 130).to_string();
        assert!(
            full.starts_with(
                " ACT  1 static 2 jolt 3 shove 4 redline 5 deadeye 6 frag×3 7 flashbang×2 8 emp puck×2 9 satchel charge×1"
            ),
            "{full:?}"
        );
        assert!(full.ends_with("AP ●● 2/2  MOV 4/4"), "{full:?}");
        assert_eq!(full.chars().count(), 130);
        let cut = act_row(&act_entries(), pools(), 100).to_string();
        assert!(cut.contains(" …"), "{cut:?}");
        assert!(!cut.contains("satchel"), "whole entries drop: {cut:?}");
        assert!(cut.ends_with("AP ●● 2/2  MOV 4/4"), "{cut:?}");
        assert_eq!(cut.chars().count(), 100);
    }

    #[test]
    fn the_act_row_colours_its_parts_and_dims_a_cooling_entry() {
        let mut entries = act_entries();
        entries[1].state = ActState::Cooling;
        entries[2].state = ActState::Refused;
        let line = act_row(&entries, pools(), 130);
        let span = |s: &str| {
            line.spans
                .iter()
                .find(|sp| sp.content == s)
                .unwrap()
                .clone()
        };
        assert_eq!(span("1 ").style.fg, Some(Color::Rgb(90, 90, 90)));
        assert_eq!(span("static").style.fg, Some(TEXT));
        assert_eq!(span("frag×3").style.fg, Some(Color::Rgb(190, 190, 140)));
        assert_eq!(span("jolt").style.fg, Some(DIM));
        assert_eq!(span(" ◌").style.fg, Some(DIM));
        assert_eq!(span("shove").style.fg, Some(DIM));
        assert!(line.to_string().contains("2 jolt ◌ 3 shove"));
    }

    #[test]
    fn the_footer_reads_the_act_row_with_the_action_points_first() {
        let f = Footer {
            place: "the lot".into(),
            saved: false,
            jacked: false,
            camera: false,
            overburdened: false,
            trace: None,
            hunted: false,
            engaged: true,
            turn: None,
            run: None,
            ice: Vec::new(),
            slots_label: "ABILITIES",
            slots: &[],
            hollow: false,
            budgets: Some((4, 4, 2, 2)),
            acting: Some(true),
            prompt: None,
            act: Some(act_entries()),
            flash_pools: false,
        };
        let rows = footer_rows(&f, 140);
        let row = rows[3].to_string();
        assert!(row.starts_with(" ACT  1 static"), "{row:?}");
        assert!(
            row.contains("AP ●● 2/2") && row.ends_with("MOV ▰▰▰▰ 4/4"),
            "{row:?}"
        );
        assert!(row.find("AP").unwrap() < row.find("MOV ").unwrap());
        let lit = Footer {
            flash_pools: true,
            ..f
        };
        let line = &footer_rows(&lit, 140)[3];
        let ap = line.spans.iter().find(|sp| sp.content == " AP ").unwrap();
        assert_eq!(ap.style.bg, Some(Color::Rgb(0, 90, 110)));
        assert_eq!(ap.style.fg, Some(Color::White));
    }

    #[test]
    fn a_construct_keeps_its_programs_and_the_abilities_follow() {
        let entries: Vec<ActEntry> = act_entries()
            .into_iter()
            .take(3)
            .enumerate()
            .map(|(i, mut e)| {
                e.key = char::from(b'6' + i as u8);
                e
            })
            .collect();
        let f = Footer {
            place: "the lot · construct".into(),
            saved: false,
            jacked: true,
            camera: false,
            overburdened: false,
            trace: None,
            hunted: false,
            engaged: true,
            turn: None,
            run: None,
            ice: Vec::new(),
            slots_label: "PROGRAMS",
            slots: &slots_for_footer(),
            hollow: false,
            budgets: Some((4, 4, 2, 2)),
            acting: Some(true),
            prompt: None,
            act: Some(entries),
            flash_pools: false,
        };
        let row = footer_rows(&f, 150)[3].to_string();
        assert!(row.starts_with(" PROGRAMS   [1] CRACK ●"), "{row:?}");
        assert!(row.contains("6 static 7 jolt 8 shove"), "{row:?}");
        assert!(row.find("[2] COPY").unwrap() < row.find("6 static").unwrap());
    }
}
