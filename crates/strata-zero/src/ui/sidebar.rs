//! The sidebar beside the map: toggleable panes sharing one column.

use crate::messages::{Entry, Kind, MessageLog};
use crate::render::glyphs::kind_icon;
use crate::render::map::{MapRef, TileMap};
use crate::render::minimap::Minimap;
use crate::render::overlay::Digital;
use crate::render::style::StylePack;
use crate::ui::chrome::{CYAN, DIM, MAGENTA, TEXT, reveal};
use crate::ui::hud::{RULE_STYLE, clock_at};
use crate::ui::panel::{inspector_lines, legend};
use crate::walker::port_name;
use citygen::{ConnectorKind, Feature, NodeKind, Pos, Rect as TileRect, StratumKind};
use game::{Character, Content, Ground};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub const MINIMAP_H: u16 = 13;
const LOG_MIN: u16 = 3;
/// The fewest rows MAP keeps (separator, a squeezed minimap, the coordinate row).
const MAP_MIN: u16 = 7;
const HERE_CAP: u16 = 7;
const QUEST_CAP: u16 = 4;
const INSPECT_CAP: u16 = 14;
const ITEM_ROWS: usize = 5;
/// Ticks the newest log entry takes to type out.
const REVEAL_TICKS: u64 = 20;
/// Cells a log line's clock stamp takes, continuation rows hang under the text.
const STAMP_W: usize = 7;

/// The right column's width: the main tab row needs 39 cells.
pub fn width(_total: u16) -> u16 {
    40
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Here,
    Quest,
    Map,
    Log,
    Inspect,
    Legend,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::Here,
        Section::Quest,
        Section::Map,
        Section::Log,
        Section::Inspect,
        Section::Legend,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Here => "here",
            Section::Quest => "quest",
            Section::Map => "map",
            Section::Log => "log",
            Section::Inspect => "inspect",
            Section::Legend => "legend",
        }
    }

    pub fn dev_only(self) -> bool {
        matches!(self, Section::Inspect | Section::Legend)
    }

    fn index(self) -> usize {
        Section::ALL
            .iter()
            .position(|s| *s == self)
            .expect("listed")
    }
}

/// Which sections are on; lives in the app state so popups can toggle them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sidebar {
    on: [bool; 6],
}

impl Default for Sidebar {
    fn default() -> Self {
        Sidebar {
            on: [true, true, true, true, false, false],
        }
    }
}

impl Sidebar {
    pub fn is_on(&self, s: Section) -> bool {
        self.on[s.index()]
    }

    pub fn set(&mut self, s: Section, on: bool) {
        self.on[s.index()] = on;
    }

    pub fn toggle(&mut self, s: Section) {
        let i = s.index();
        self.on[i] = !self.on[i];
    }

    /// Turns the dev-only sections off.
    pub fn leave_dev(&mut self) {
        for s in Section::ALL {
            if s.dev_only() {
                self.set(s, false);
            }
        }
    }

    /// The sections to draw, in order; dev sections only in dev mode.
    pub fn active(&self, dev: bool) -> Vec<Section> {
        Section::ALL
            .into_iter()
            .filter(|s| self.is_on(*s) && (dev || !s.dev_only()))
            .collect()
    }
}

/// Rows per pane, in order; needs include each pane's separator row, which every
/// pane but the first carries. Each non-LOG pane takes the smaller of its need and
/// what is left; LOG takes the remainder. When LOG would get under three rows, MAP
/// gives up rows first, down to seven, and drops only when that is not enough. A pane
/// that would get only its separator row gets nothing instead.
pub fn budget(needs: &[(Section, u16)], height: u16) -> Vec<(Section, u16)> {
    let mut needs: Vec<(Section, u16)> = needs.to_vec();
    // The first pane has no separator: one row is a whole pane there, two elsewhere.
    let whole = |i: usize, r: u16| if r < if i == 0 { 1 } else { 2 } { 0 } else { r };
    loop {
        let mut left = height;
        let mut rows: Vec<(Section, u16)> = Vec::with_capacity(needs.len());
        for (i, (s, n)) in needs.iter().enumerate() {
            if *s == Section::Log {
                rows.push((*s, 0));
                continue;
            }
            let r = whole(i, (*n).min(left));
            left -= r;
            rows.push((*s, r));
        }
        let has_log = needs.iter().any(|(s, _)| *s == Section::Log);
        let map_at = needs.iter().position(|(s, _)| *s == Section::Map);
        if has_log
            && left < LOG_MIN
            && let Some(i) = map_at
        {
            let short = LOG_MIN - left;
            if rows[i].1 >= MAP_MIN + short {
                rows[i].1 -= short;
                left += short;
            } else {
                needs.retain(|(s, _)| *s != Section::Map);
                continue;
            }
        }
        if let Some(i) = needs.iter().position(|(s, _)| *s == Section::Log) {
            rows[i].1 = whole(i, left);
        }
        rows.retain(|(_, r)| *r > 0);
        return rows;
    }
}

pub struct SidebarView<'a> {
    pub map: MapRef<'a>,
    pub focus: Pos,
    /// The main viewport in tiles, for the minimap.
    pub view: TileRect,
    pub player: Option<Pos>,
    pub style: &'a StylePack,
    pub content: &'a Content,
    pub character: &'a Character,
    pub ground: &'a Ground,
    pub log: &'a MessageLog,
    pub tick: u64,
    pub dev: bool,
    /// Whoever stands on the focus tile, for the inspector.
    pub person: Option<&'a sim::Card>,
    /// The Net's state: the trace and the hacks inside a construct.
    pub net: &'a crate::net::NetState,
    /// The game minute, for the trace's alert and the hacks' windows.
    pub now: u64,
    /// The ICE the player can see in their construct, one line each.
    pub ice_lines: Vec<String>,
}

fn plain(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(TEXT)))
}

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

fn separator(width: u16) -> Line<'static> {
    Line::from(Span::styled("─".repeat(width as usize), RULE_STYLE))
}

/// The focus tile: its feature, then what lies on it.
pub fn here_lines(v: &SidebarView) -> Vec<Line<'static>> {
    let city = v.map.city();
    let mut out = Vec::new();
    if let citygen::Place::Construct(b) = v.map.place() {
        for (i, line) in crate::net::trace_lines(v.net, b, v.now)
            .into_iter()
            .enumerate()
        {
            let style = if i == 0 {
                Style::default().fg(CYAN)
            } else {
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
            };
            out.push(Line::from(Span::styled(format!(" {line}"), style)));
        }
        for line in &v.ice_lines {
            out.push(Line::from(Span::styled(
                format!(" {line}"),
                Style::default().fg(MAGENTA),
            )));
        }
        if let Some(t) = v.map.tile(v.focus) {
            match t.feature {
                Some(Feature::Node(k)) => {
                    // The node under the focus, not any node of its kind.
                    let id = match &v.map {
                        MapRef::Construct(m) => m.construct.node_at(v.focus),
                        _ => None,
                    };
                    let state = match id {
                        Some(id) if v.net.blinded(id, v.now) => " · blind",
                        Some(id) if v.net.copied(id, v.now) => " · copied",
                        _ => "",
                    };
                    out.push(plain(format!(" ◆ {}{state}", k.label())));
                }
                Some(Feature::Ice(g)) => out.push(plain(format!(" ▣ ICE · {}", g.label()))),
                _ => {}
            }
        }
    } else if v.map.stratum() == Some(StratumKind::Net) {
        if let Some(id) = v
            .map
            .stratum()
            .and_then(|k| Digital::node_under(city, k, v.focus))
        {
            out.push(plain(format!(" ◆ {}", city.net.node(id).kind.label())));
        }
    } else if let Some(t) = v.map.tile(v.focus)
        && t.feature == Some(Feature::Node(NodeKind::Terminal))
    {
        out.push(plain(format!(" ╥ {}", port_name(city, v.focus))));
    }
    for c in city
        .connectors_at(v.focus)
        .filter(|c| c.kind != ConnectorKind::Jack)
    {
        let Some(k) = v.map.stratum() else {
            continue;
        };
        let Some((below, above)) = c.serves_from(k) else {
            continue;
        };
        if above.is_some() {
            out.push(plain(format!(" > {} up", c.kind.label())));
        }
        if below.is_some() {
            out.push(plain(format!(" < {} down", c.kind.label())));
        }
    }
    let stacks = v.ground.at(v.map.place(), v.focus);
    for s in stacks.iter().take(ITEM_ROWS) {
        let (name, icon) = match v.content.item(&s.item) {
            Some(i) => (i.name.clone(), kind_icon(i.kind)),
            None => (s.item.to_string(), '?'),
        };
        let qty = if s.qty > 1 {
            format!(" ×{}", s.qty)
        } else {
            String::new()
        };
        out.push(plain(format!(" {icon} {name}{qty}")));
    }
    if stacks.len() > ITEM_ROWS {
        out.push(dim(format!(" … and {} more", stacks.len() - ITEM_ROWS)));
    }
    if out.is_empty() {
        out.push(dim(" nothing here"));
    }
    out
}

/// One line for the footer: `3 items · terminal · stairs`, or `nothing`.
pub fn here_summary(v: &SidebarView) -> String {
    let mut parts = Vec::new();
    let n = v.ground.at(v.map.place(), v.focus).len();
    if n == 1 {
        parts.push("1 item".to_string());
    } else if n > 1 {
        parts.push(format!("{n} items"));
    }
    if v.map.stratum() == Some(StratumKind::Net) {
        if let Some(id) = v
            .map
            .stratum()
            .and_then(|k| Digital::node_under(v.map.city(), k, v.focus))
        {
            parts.push(v.map.city().net.node(id).kind.label().to_string());
        }
    } else if v
        .map
        .tile(v.focus)
        .is_some_and(|t| t.feature == Some(Feature::Node(NodeKind::Terminal)))
    {
        parts.push("terminal".to_string());
    }
    for c in v.map.city().connectors_at(v.focus).filter(|c| {
        c.kind != ConnectorKind::Jack && v.map.stratum().is_some_and(|k| c.serves.contains(&k))
    }) {
        parts.push(c.kind.label().to_string());
    }
    if parts.is_empty() {
        "nothing".to_string()
    } else {
        parts.join(" · ")
    }
}

/// The first unfinished quest: title and giver, its first open objective, and pips.
pub fn quest_lines(character: &Character, content: &Content) -> Vec<Line<'static>> {
    for q in &character.quests {
        if q.is_done() {
            continue;
        }
        let Some(quest) = content.quest(&q.quest) else {
            continue;
        };
        let Some(i) = q.done.iter().position(|d| !*d) else {
            continue;
        };
        let done = q.done.iter().filter(|d| **d).count();
        let total = q.done.len();
        let objective = quest.objectives.get(i).cloned().unwrap_or_default();
        return vec![
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    quest.title.to_uppercase(),
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" · {}", quest.giver), Style::default().fg(TEXT)),
            ]),
            plain(format!(" » {objective}")),
            Line::from(vec![
                Span::raw(" "),
                Span::styled("▰".repeat(done), Style::default().fg(CYAN)),
                Span::styled("▱".repeat(total - done), Style::default().fg(DIM)),
                Span::styled(format!(" {done}/{total}"), Style::default().fg(DIM)),
            ]),
        ];
    }
    vec![dim(" no active quest")]
}

/// Word-wraps `text` to `width` cells; words longer than a row are broken.
pub(crate) fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut row_w = 0usize;
    for word in text.split(' ') {
        let mut word: Vec<char> = word.chars().collect();
        while !word.is_empty() {
            let sep = if row_w == 0 { 0 } else { 1 };
            if row_w + sep + word.len() <= width {
                if sep == 1 {
                    row.push(' ');
                }
                row.extend(word.iter());
                row_w += sep + word.len();
                word.clear();
            } else if row_w == 0 {
                let take = width.min(word.len());
                row.extend(word.iter().take(take));
                rows.push(std::mem::take(&mut row));
                word.drain(..take);
            } else {
                rows.push(std::mem::take(&mut row));
                row_w = 0;
            }
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

/// A log entry: its clock stamp, then the marked text; wraps hang under the text.
fn entry_lines(e: &Entry, width: u16) -> Vec<Line<'static>> {
    let style = match e.kind {
        Kind::Info => Style::default().fg(TEXT),
        Kind::Warn => Style::default().fg(MAGENTA),
        Kind::Story => Style::default().fg(CYAN),
    };
    let lit = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
    let (h, m) = clock_at(e.tick);
    let stamp = format!(" {h:02}:{m:02} ");
    // The kind's marker shifts every name by its width.
    let marked = e.marked();
    let shift = marked.chars().count() - e.text.chars().count();
    let is_lit = |i: usize| {
        i >= shift
            && e.marks
                .iter()
                .any(|(a, b)| (a + shift..b + shift).contains(&i))
    };
    let chars: Vec<char> = marked.chars().collect();
    let mut pos = 0usize;
    wrap_text(&marked, (width as usize).saturating_sub(STAMP_W))
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            // Find the row's chars in the text from where the last row ended; wrapping
            // only drops spaces between rows.
            let row_chars: Vec<char> = row.chars().collect();
            let mut start = pos;
            while start + row_chars.len() <= chars.len()
                && chars[start..start + row_chars.len()] != row_chars[..]
            {
                start += 1;
            }
            if start + row_chars.len() > chars.len() {
                start = pos;
            }
            let mut spans = vec![if i == 0 {
                Span::styled(stamp.clone(), Style::default().fg(DIM))
            } else {
                Span::raw(" ".repeat(STAMP_W))
            }];
            let mut run = String::new();
            let mut run_lit = false;
            for (k, ch) in row_chars.iter().enumerate() {
                let on = is_lit(start + k);
                if on != run_lit && !run.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut run),
                        if run_lit { lit } else { style },
                    ));
                }
                run_lit = on;
                run.push(*ch);
            }
            if !run.is_empty() {
                spans.push(Span::styled(run, if run_lit { lit } else { style }));
            }
            pos = start + row_chars.len();
            Line::from(spans)
        })
        .collect()
}

/// Rows `lines` take at `width` when wrapped; at least one per line.
fn rows_for(lines: &[Line<'static>], width: u16) -> u16 {
    lines
        .iter()
        .map(|l| wrap_text(&l.to_string(), width as usize).len() as u16)
        .sum()
}

/// Whole entries from the newest that fit `area`, oldest at the top; the newest reveals.
fn draw_log(log: &MessageLog, tick: u64, area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let mut chunks: Vec<Vec<Line<'static>>> = Vec::new();
    let mut rows = 0u16;
    for e in log.iter().rev() {
        let lines = entry_lines(e, area.width);
        if rows + lines.len() as u16 > area.height {
            break;
        }
        rows += lines.len() as u16;
        chunks.push(lines);
    }
    chunks.reverse();
    let n = chunks.len();
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, lines) in chunks.into_iter().enumerate() {
        if i + 1 == n
            && let Some(e) = log.latest()
        {
            let p = tick.saturating_sub(e.tick) as f32 / REVEAL_TICKS as f32;
            out.extend(reveal(&lines, p, (tick / 4).is_multiple_of(2)));
        } else {
            out.extend(lines);
        }
    }
    Paragraph::new(out).render(area, buf);
}

/// Draws the column: the active panes in order within their budget, a light rule
/// before every pane but the first. Returns the panes that got rows, so the tabs can
/// dim a squeezed-out one and the map can toast when LOG is missing.
pub fn draw(v: &SidebarView, flags: &Sidebar, area: Rect, buf: &mut Buffer) -> Vec<Section> {
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    let w = area.width;
    let node = v
        .map
        .stratum()
        .and_then(|k| Digital::node_under(v.map.city(), k, v.focus))
        .map(|id| v.map.city().net.node(id));
    let mut built: Vec<(Section, Vec<Line<'static>>, u16)> = Vec::new();
    for (i, s) in flags.active(v.dev).into_iter().enumerate() {
        let sep = if i == 0 { 0 } else { 1 };
        let (lines, need) = match s {
            Section::Here => {
                let l = here_lines(v);
                let n = rows_for(&l, w).min(HERE_CAP);
                (l, n)
            }
            Section::Quest => {
                let l = quest_lines(v.character, v.content);
                let n = rows_for(&l, w).min(QUEST_CAP);
                (l, n)
            }
            Section::Map => (Vec::new(), MINIMAP_H + 1),
            Section::Log => (Vec::new(), 0),
            Section::Inspect => {
                let l: Vec<Line<'static>> = inspector_lines(
                    &v.map,
                    v.focus,
                    node,
                    v.ground.at(v.map.place(), v.focus),
                    v.content,
                    v.person,
                )
                .into_iter()
                .map(|l| {
                    let mut spans = vec![Span::raw(" ")];
                    spans.extend(l.spans);
                    Line::from(spans)
                })
                .collect();
                let n = rows_for(&l, w).min(INSPECT_CAP);
                (l, n)
            }
            Section::Legend => {
                let l = legend(v.map.stratum().unwrap_or(StratumKind::Street));
                let n = l.len() as u16;
                (l, n)
            }
        };
        built.push((s, lines, need + sep));
    }
    let needs: Vec<(Section, u16)> = built.iter().map(|(s, _, n)| (*s, *n)).collect();
    let mut y = area.y;
    let mut drawn = Vec::new();
    let mut first = true;
    for (s, mut rows) in budget(&needs, area.height) {
        drawn.push(s);
        if !first {
            Paragraph::new(separator(w)).render(Rect::new(area.x, y, w, 1), buf);
            y += 1;
            rows -= 1;
        }
        first = false;
        let (_, lines, _) = built.iter().find(|(x, ..)| *x == s).expect("budgeted");
        let content = Rect::new(area.x, y, w, rows);
        match s {
            Section::Map => {
                let map_rows = rows.saturating_sub(1);
                Minimap {
                    map: &v.map,
                    view: v.view,
                    player: v.player,
                    style: v.style,
                }
                .render(Rect::new(area.x, y, w, map_rows), buf);
                let coord = format!("{},{} · {} ", v.focus.x, v.focus.y, v.map.label());
                let pad = (w as usize).saturating_sub(coord.chars().count());
                Paragraph::new(Line::from(Span::styled(
                    format!("{}{coord}", " ".repeat(pad)),
                    Style::default().fg(DIM),
                )))
                .render(Rect::new(area.x, y + map_rows, w, rows - map_rows), buf);
            }
            Section::Log => draw_log(v.log, v.tick, content, buf),
            _ => Paragraph::new(lines.clone())
                .wrap(Wrap { trim: false })
                .render(content, buf),
        }
        y += rows;
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::screens::testutil::buffer_text;
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        st.city = Some(city);
        st
    }

    fn view<'a>(st: &'a AppState, focus: Pos, place: citygen::Place) -> SidebarView<'a> {
        SidebarView {
            map: MapRef::for_place(
                st.city.as_ref().unwrap(),
                &st.interiors,
                &st.constructs,
                place,
            )
            .unwrap(),
            focus,
            view: TileRect::new(0, 0, 70, 25),
            player: Some(focus),
            style: &st.style,
            content: &st.content,
            character: &st.character,
            ground: &st.ground,
            log: &st.log,
            tick: st.tick,
            dev: st.dev,
            person: None,
            net: &st.net,
            now: 0,
            ice_lines: Vec::new(),
        }
    }

    fn draw_to(v: &SidebarView, flags: &Sidebar, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| {
            draw(v, flags, f.area(), f.buffer_mut());
        })
        .unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn flags_default_and_toggle() {
        let mut s = Sidebar::default();
        assert!(s.is_on(Section::Here) && s.is_on(Section::Log) && !s.is_on(Section::Inspect));
        s.toggle(Section::Here);
        assert!(!s.is_on(Section::Here));
        s.set(Section::Inspect, true);
        s.set(Section::Legend, true);
        assert_eq!(
            s.active(false),
            vec![Section::Quest, Section::Map, Section::Log]
        );
        assert_eq!(
            s.active(true),
            vec![
                Section::Quest,
                Section::Map,
                Section::Log,
                Section::Inspect,
                Section::Legend
            ]
        );
        s.leave_dev();
        assert!(!s.is_on(Section::Inspect) && !s.is_on(Section::Legend));
    }

    #[test]
    fn budget_gives_log_the_rest_and_drops_map_first() {
        use Section::*;
        assert_eq!(
            budget(&[(Here, 3), (Quest, 4), (Map, 14), (Log, 0)], 25),
            vec![(Here, 3), (Quest, 4), (Map, 14), (Log, 4)]
        );
        assert_eq!(
            budget(&[(Here, 3), (Quest, 4), (Map, 14), (Log, 0)], 22),
            vec![(Here, 3), (Quest, 4), (Map, 12), (Log, 3)]
        );
        assert_eq!(
            budget(&[(Here, 3), (Quest, 4), (Map, 14)], 10),
            vec![(Here, 3), (Quest, 4), (Map, 3)]
        );
        assert_eq!(
            budget(&[(Here, 3), (Log, 0), (Inspect, 15)], 20),
            vec![(Here, 3), (Log, 2), (Inspect, 15)]
        );
        assert_eq!(budget(&[(Here, 3), (Log, 0)], 3), vec![(Here, 3)]);
        // MAP gives up rows to keep LOG at its floor, down to its own floor of seven.
        assert_eq!(
            budget(&[(Here, 2), (Quest, 4), (Map, 15), (Log, 0)], 22),
            vec![(Here, 2), (Quest, 4), (Map, 13), (Log, 3)]
        );
        assert_eq!(
            budget(&[(Here, 2), (Quest, 4), (Map, 15), (Log, 0)], 16),
            vec![(Here, 2), (Quest, 4), (Map, 7), (Log, 3)]
        );
        assert_eq!(
            budget(&[(Here, 2), (Quest, 4), (Map, 15), (Log, 0)], 15),
            vec![(Here, 2), (Quest, 4), (Log, 9)]
        );
        for h in 0..40u16 {
            let rows = budget(
                &[
                    (Here, 3),
                    (Quest, 4),
                    (Map, 14),
                    (Log, 0),
                    (Inspect, 15),
                    (Legend, 9),
                ],
                h,
            );
            assert!(rows.iter().map(|(_, r)| r).sum::<u16>() <= h);
            assert!(rows.iter().all(|(_, r)| *r > 0));
        }
    }

    #[test]
    fn here_lists_feature_and_items_capped() {
        let mut st = state();
        let k = citygen::Place::Stratum(StratumKind::Street);
        // A plain street tile: walkable, no connector, so HERE starts empty.
        let pos = {
            let city = st.city.as_ref().unwrap();
            (0..city.size.h)
                .flat_map(|y| (0..city.size.w).map(move |x| Pos::new(x as i32, y as i32)))
                .find(|p| {
                    city.walkable(k.stratum().unwrap(), *p)
                        && city.connectors_at(*p).next().is_none()
                        && city
                            .tile(k.stratum().unwrap(), *p)
                            .is_some_and(|t| t.feature.is_none())
                })
                .unwrap()
        };
        let text = |st: &AppState| {
            here_lines(&view(st, pos, k))
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(text(&st), vec![" nothing here"]);
        for (i, id) in [
            "copper_coil",
            "dead_phone",
            "stim_patch",
            "soy_noodle_cup",
            "scrap_servo",
            "bottle_caps",
            "burner_shard",
        ]
        .iter()
        .enumerate()
        {
            st.ground.add(k, pos, game::ItemId::new(id), i as u32 + 1);
        }
        let lines = text(&st);
        let coil = kind_icon(game::ItemKind::Component);
        assert_eq!(lines[0], format!(" {coil} Copper coil"));
        assert_eq!(
            lines[1],
            format!(" {} Dead phone ×2", kind_icon(game::ItemKind::Junk))
        );
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert_eq!(lines[5], " … and 2 more");
        // The player spawns on a public connector: the stairs show there.
        let spawn = st.player.as_ref().unwrap().pos;
        let city = st.city.as_ref().unwrap();
        let conn = city
            .connectors_at(spawn)
            .find(|c| c.serves.contains(&k.stratum().unwrap()))
            .unwrap();
        let all: Vec<String> = here_lines(&view(&st, spawn, k))
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert!(all.iter().any(|l| l.contains(conn.kind.label())), "{all:?}");
    }

    #[test]
    fn quest_shows_title_objective_and_pips() {
        let st = state();
        let lines: Vec<String> = quest_lines(&st.character, &st.content)
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert_eq!(
            lines,
            vec![
                " THE DROP · Mama Sol",
                " » Cross the corp core without a scan",
                " ▰▱▱ 1/3"
            ]
        );
        let mut ch = st.character.clone();
        for q in &mut ch.quests {
            q.done.fill(true);
        }
        let lines: Vec<String> = quest_lines(&ch, &st.content)
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert_eq!(lines, vec![" no active quest"]);
    }

    #[test]
    fn panes_are_separated_by_light_rules_and_have_no_titles() {
        let mut st = state();
        st.dev = false;
        st.tick += 200;
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        let text = draw_to(&view(&st, pos, k), &st.sidebar, width(120), 40);
        assert!(
            !text.contains("HERE")
                && !text.contains("LOG")
                && !text.contains("VEX")
                && !text.contains('╭'),
            "{text}"
        );
        assert_eq!(text.matches(&"─".repeat(40)).count(), 3, "{text}");
        assert!(text.contains("· Street"), "coordinate row: {text}");
        assert_eq!(width(100), 40);
        assert_eq!(width(120), 40);
    }

    #[test]
    fn first_pane_has_no_separator() {
        let mut st = state();
        st.dev = false;
        st.tick = 90 * 6;
        st.say("hello");
        st.tick += 200;
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        let mut flags = Sidebar::default();
        for s in [Section::Here, Section::Quest, Section::Map] {
            flags.set(s, false);
        }
        let text = draw_to(&view(&st, pos, k), &flags, width(100), 10);
        let first = text.lines().next().unwrap();
        assert!(first.starts_with(" 22:06 hello"), "{text}");
        assert!(!text.contains('─'), "{text}");
    }

    #[test]
    fn log_lines_carry_stamps_and_hang_wraps() {
        let mut st = state();
        st.dev = false;
        st.tick = 90 * 3;
        st.say("x".repeat(50));
        st.tick += 200;
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        let mut flags = Sidebar::default();
        for s in [Section::Here, Section::Quest, Section::Map] {
            flags.set(s, false);
        }
        let text = draw_to(&view(&st, pos, k), &flags, width(100), 10);
        let rows: Vec<&str> = text.lines().filter(|l| l.contains('x')).collect();
        assert!(rows[0].starts_with(" 22:03 x"), "{text}");
        assert!(rows[1].starts_with("       x"), "{text}");
        assert_eq!(text.matches('x').count(), 50);
        assert!(!here_summary(&view(&st, pos, k)).is_empty());
    }

    #[test]
    fn draw_fits_every_section_and_wraps_log_entries() {
        let mut st = state();
        st.dev = true;
        st.sidebar.set(Section::Inspect, true);
        st.sidebar.set(Section::Legend, true);
        st.say("a".repeat(60));
        st.tick += 200;
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        for h in [1u16, 2, 10, 25, 40] {
            let text = draw_to(&view(&st, pos, k), &st.sidebar, width(100), h);
            assert_eq!(text.lines().count(), h as usize);
        }
        let text = draw_to(&view(&st, pos, k), &st.sidebar, width(100), 60);
        for needle in [
            "THE DROP · Mama Sol",
            "▰▱▱ 1/3",
            "· Street",
            "aaaa",
            "pos ",
            "terrain",
        ] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        // Six panes on: five separators.
        assert_eq!(text.matches(&"─".repeat(40)).count(), 5, "{text}");
        assert_eq!(text.lines().count(), 60);
        assert_eq!(
            budget(&[(Section::Here, 1), (Section::Log, 0)], 1),
            vec![(Section::Here, 1)]
        );
        assert_eq!(
            budget(&[(Section::Here, 3), (Section::Quest, 2)], 4),
            vec![(Section::Here, 3)]
        );
    }

    #[test]
    fn log_wraps_long_entries_whole() {
        let mut st = state();
        st.dev = false;
        st.say("x".repeat(70));
        st.say("short");
        st.tick += 200;
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        let mut flags = Sidebar::default();
        for s in [Section::Here, Section::Quest, Section::Map] {
            flags.set(s, false);
        }
        // LOG is the only pane: 6 rows; the 70-cell entry wraps to 3 rows at 33 cells, "short" 1.
        let text = draw_to(&view(&st, pos, k), &flags, width(100), 6);
        assert_eq!(text.matches('x').count(), 70, "{text}");
        assert!(text.contains("short"), "{text}");
        // Four rows: the long entry does not fit whole, only "short" shows.
        let text = draw_to(&view(&st, pos, k), &flags, width(100), 3);
        assert_eq!(text.matches('x').count(), 0, "{text}");
        assert!(text.contains("short"), "{text}");
    }

    #[test]
    fn here_summary_names_the_net_node() {
        let st = state();
        let city = st.city.as_ref().unwrap();
        let node = &city.net.nodes[0];
        let v = view(&st, node.cyber, citygen::Place::Stratum(StratumKind::Net));
        assert!(here_summary(&v).contains(node.kind.label()));
    }

    #[test]
    fn draw_reports_the_panes_it_drew() {
        let mut st = state();
        st.dev = true;
        st.tick += 200;
        for s in [Section::Inspect, Section::Legend] {
            st.sidebar.set(s, true);
        }
        st.sidebar.set(Section::Log, false);
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        let mut t = Terminal::new(TestBackend::new(width(100), 22)).unwrap();
        let mut drawn = Vec::new();
        t.draw(|f| {
            drawn = draw(&view(&st, pos, k), &st.sidebar, f.area(), f.buffer_mut());
        })
        .unwrap();
        assert!(
            drawn.contains(&Section::Map) && !drawn.contains(&Section::Legend),
            "{drawn:?}"
        );
    }

    #[test]
    fn a_named_log_entry_lights_its_names() {
        let mut log = MessageLog::default();
        log.push(Kind::Info, "Entering `Novbio Labs`", 0);
        let lines = entry_lines(log.latest().unwrap(), 40);
        let spans: Vec<(String, bool)> = lines[0]
            .spans
            .iter()
            .map(|s| {
                (
                    s.content.to_string(),
                    s.style.fg == Some(CYAN) && s.style.add_modifier.contains(Modifier::BOLD),
                )
            })
            .collect();
        assert_eq!(spans[1], ("Entering ".to_string(), false), "{spans:?}");
        assert_eq!(spans[2], ("Novbio Labs".to_string(), true), "{spans:?}");
        // A name that wraps keeps its light on both rows.
        log.push(Kind::Info, "lock cracked · `Hextek Holdings`'s doors", 0);
        let lines = entry_lines(log.latest().unwrap(), 26);
        assert!(lines.len() >= 2, "{lines:?}");
        let lit = |l: &Line<'static>| {
            l.spans
                .iter()
                .filter(|s| s.style.fg == Some(CYAN))
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        let all: String = lines.iter().map(lit).collect();
        assert_eq!(all.replace(' ', ""), "HextekHoldings", "{lines:?}");
    }
}
