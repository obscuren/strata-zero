//! The sidebar beside the map: toggleable sections sharing one column.

use crate::messages::{Entry, Kind, MessageLog};
use crate::render::glyphs::kind_icon;
use crate::render::minimap::Minimap;
use crate::render::overlay::Digital;
use crate::render::style::StylePack;
use crate::ui::chrome::{CYAN, DIM, MAGENTA, RULE, TEXT, reveal};
use crate::ui::panel::{boxed, inspector_lines, legend};
use crate::walker::port_name;
use citygen::{City, ConnectorKind, Feature, NodeKind, Pos, Rect as TileRect, StratumKind};
use game::{Character, Content, Ground};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub const WIDTH: u16 = 30;
pub const MINIMAP_H: u16 = 13;
const LOG_MIN: u16 = 3;
const HERE_CAP: u16 = 8;
const QUEST_CAP: u16 = 4;
const INSPECT_CAP: u16 = 15;
const ITEM_ROWS: usize = 5;
/// Ticks the newest log entry takes to type out.
const REVEAL_TICKS: u64 = 20;

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

/// Rows per section, in order. Each non-LOG section takes the smaller of its need and
/// what is left; LOG takes the remainder; MAP drops when LOG would get under three rows.
/// A section that would get only its rule row gets nothing instead.
pub fn budget(needs: &[(Section, u16)], height: u16) -> Vec<(Section, u16)> {
    let mut needs: Vec<(Section, u16)> = needs.to_vec();
    let whole = |r: u16| if r < 2 { 0 } else { r };
    loop {
        let mut left = height;
        let mut rows: Vec<(Section, u16)> = Vec::with_capacity(needs.len());
        for (s, n) in &needs {
            if *s == Section::Log {
                rows.push((*s, 0));
                continue;
            }
            let r = whole((*n).min(left));
            left -= r;
            rows.push((*s, r));
        }
        let has_log = needs.iter().any(|(s, _)| *s == Section::Log);
        let has_map = needs.iter().any(|(s, _)| *s == Section::Map);
        if has_log && left < LOG_MIN && has_map {
            needs.retain(|(s, _)| *s != Section::Map);
            continue;
        }
        if let Some(r) = rows.iter_mut().find(|(s, _)| *s == Section::Log) {
            r.1 = whole(left);
        }
        rows.retain(|(_, r)| *r > 0);
        return rows;
    }
}

pub struct SidebarView<'a> {
    pub city: &'a City,
    pub stratum: StratumKind,
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
}

fn plain(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(TEXT)))
}

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

/// `── TITLE ───…` padded to `width`.
fn rule(title: &str, width: u16) -> Line<'static> {
    let head = format!("── {title} ");
    let n = (width as usize).saturating_sub(head.chars().count());
    Line::from(vec![
        Span::styled("── ", Style::default().fg(RULE)),
        Span::styled(
            format!("{title} "),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("─".repeat(n), Style::default().fg(RULE)),
    ])
}

/// The focus tile: its feature, then what lies on it.
pub fn here_lines(v: &SidebarView) -> Vec<Line<'static>> {
    let city = v.city;
    let mut out = Vec::new();
    if v.stratum == StratumKind::Net {
        if let Some(id) = Digital::node_under(city, v.stratum, v.focus) {
            out.push(plain(format!("◆ {}", city.net.node(id).kind.label())));
        }
    } else if let Some(t) = city.tile(v.stratum, v.focus)
        && t.feature == Some(Feature::Node(NodeKind::Terminal))
    {
        out.push(plain(format!("╥ {}", port_name(city, v.focus))));
    }
    for c in city
        .connectors_at(v.focus)
        .filter(|c| c.kind != ConnectorKind::Jack)
    {
        let Some((below, above)) = c.serves_from(v.stratum) else {
            continue;
        };
        if above.is_some() {
            out.push(plain(format!("> {} up", c.kind.label())));
        }
        if below.is_some() {
            out.push(plain(format!("< {} down", c.kind.label())));
        }
    }
    let stacks = v.ground.at(v.stratum, v.focus);
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
        out.push(plain(format!("{icon} {name}{qty}")));
    }
    if stacks.len() > ITEM_ROWS {
        out.push(dim(format!("… and {} more", stacks.len() - ITEM_ROWS)));
    }
    if out.is_empty() {
        out.push(dim("nothing here"));
    }
    out
}

/// The first unfinished quest's upper-cased title and first open objective.
pub fn quest_lines(character: &Character, content: &Content) -> (String, Vec<Line<'static>>) {
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
        let objective = quest.objectives.get(i).cloned().unwrap_or_default();
        return (quest.title.to_uppercase(), vec![plain(objective)]);
    }
    ("QUEST".to_string(), vec![dim("no active quest")])
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

fn entry_lines(e: &Entry, width: u16) -> Vec<Line<'static>> {
    let style = match e.kind {
        Kind::Info => Style::default().fg(TEXT),
        Kind::Warn => Style::default().fg(MAGENTA),
        Kind::Story => Style::default().fg(CYAN),
    };
    wrap_text(&e.marked(), width as usize)
        .into_iter()
        .map(|s| Line::from(Span::styled(s, style)))
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

fn list_line(flags: &Sidebar, dev_row: bool) -> Line<'static> {
    let mut spans = Vec::new();
    for s in Section::ALL.iter().filter(|s| s.dev_only() == dev_row) {
        let on = flags.is_on(*s);
        spans.push(Span::styled(
            if on { " ▣ " } else { " ▢ " },
            Style::default().fg(if on { CYAN } else { DIM }),
        ));
        spans.push(Span::styled(
            s.label().to_string(),
            Style::default().fg(if on { TEXT } else { DIM }),
        ));
    }
    Line::from(spans)
}

/// Draws the panel: the section list, then each active section within its budget.
pub fn draw(v: &SidebarView, flags: &Sidebar, area: Rect, buf: &mut Buffer) {
    let title = v.character.handle.to_uppercase();
    let block = boxed(&title);
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let w = inner.width;
    let mut list = vec![list_line(flags, false)];
    if v.dev {
        list.push(list_line(flags, true));
    }
    let list_h = (list.len() as u16).min(inner.height);
    Paragraph::new(list).render(Rect::new(inner.x, inner.y, w, list_h), buf);
    let body = Rect::new(inner.x, inner.y + list_h, w, inner.height - list_h);
    let node = Digital::node_under(v.city, v.stratum, v.focus).map(|id| v.city.net.node(id));
    let mut built: Vec<(Section, String, Vec<Line<'static>>, u16)> = Vec::new();
    for s in flags.active(v.dev) {
        let (title, lines, need) = match s {
            Section::Here => {
                let l = here_lines(v);
                let n = rows_for(&l, w).min(HERE_CAP - 1) + 1;
                ("HERE".to_string(), l, n)
            }
            Section::Quest => {
                let (t, l) = quest_lines(v.character, v.content);
                let n = rows_for(&l, w).min(QUEST_CAP - 1) + 1;
                (t, l, n)
            }
            Section::Map => (
                format!("MAP · {}", v.stratum.label().to_lowercase()),
                Vec::new(),
                MINIMAP_H + 1,
            ),
            Section::Log => ("LOG".to_string(), Vec::new(), 0),
            Section::Inspect => {
                let l = inspector_lines(
                    v.city,
                    v.stratum,
                    v.focus,
                    node,
                    v.ground.at(v.stratum, v.focus),
                    v.content,
                );
                let n = rows_for(&l, w).min(INSPECT_CAP - 1) + 1;
                ("INSPECT".to_string(), l, n)
            }
            Section::Legend => {
                let l = legend(v.stratum);
                let n = l.len() as u16 + 1;
                ("LEGEND".to_string(), l, n)
            }
        };
        built.push((s, title, lines, need));
    }
    let needs: Vec<(Section, u16)> = built.iter().map(|(s, _, _, n)| (*s, *n)).collect();
    let mut y = body.y;
    for (s, rows) in budget(&needs, body.height) {
        let (_, title, lines, _) = built.iter().find(|(x, ..)| *x == s).expect("budgeted");
        Paragraph::new(rule(title, w)).render(Rect::new(body.x, y, w, 1), buf);
        let content = Rect::new(body.x, y + 1, w, rows - 1);
        match s {
            Section::Map => Minimap {
                city: v.city,
                stratum: v.stratum,
                view: v.view,
                player: v.player,
                style: v.style,
            }
            .render(content, buf),
            Section::Log => draw_log(v.log, v.tick, content, buf),
            _ => Paragraph::new(lines.clone())
                .wrap(Wrap { trim: false })
                .render(content, buf),
        }
        y += rows;
    }
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

    fn view<'a>(st: &'a AppState, focus: Pos, stratum: StratumKind) -> SidebarView<'a> {
        SidebarView {
            city: st.city.as_ref().unwrap(),
            stratum,
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
        }
    }

    fn draw_to(v: &SidebarView, flags: &Sidebar, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(v, flags, f.area(), f.buffer_mut()))
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
            vec![(Here, 3), (Quest, 4), (Log, 15)]
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
        let k = StratumKind::Street;
        // A plain street tile: walkable, no connector, so HERE starts empty.
        let pos = {
            let city = st.city.as_ref().unwrap();
            (0..city.size.h)
                .flat_map(|y| (0..city.size.w).map(move |x| Pos::new(x as i32, y as i32)))
                .find(|p| {
                    city.walkable(k, *p)
                        && city.connectors_at(*p).next().is_none()
                        && city.tile(k, *p).is_some_and(|t| t.feature.is_none())
                })
                .unwrap()
        };
        let text = |st: &AppState| {
            here_lines(&view(st, pos, k))
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(text(&st), vec!["nothing here"]);
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
        assert_eq!(lines[0], format!("{coil} Copper coil"));
        assert_eq!(
            lines[1],
            format!("{} Dead phone ×2", kind_icon(game::ItemKind::Junk))
        );
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert_eq!(lines[5], "… and 2 more");
        // The player spawns on a public connector: the stairs show there.
        let spawn = st.player.as_ref().unwrap().pos;
        let city = st.city.as_ref().unwrap();
        let conn = city
            .connectors_at(spawn)
            .find(|c| c.serves.contains(&k))
            .unwrap();
        let all: Vec<String> = here_lines(&view(&st, spawn, k))
            .iter()
            .map(|l| l.to_string())
            .collect();
        assert!(all.iter().any(|l| l.contains(conn.kind.label())), "{all:?}");
    }

    #[test]
    fn quest_picks_the_first_open_objective() {
        let st = state();
        let (title, lines) = quest_lines(&st.character, &st.content);
        assert_eq!(title, "THE DROP");
        assert_eq!(lines[0].to_string(), "Cross the corp core without a scan");
        let mut ch = st.character.clone();
        for q in &mut ch.quests {
            q.done.fill(true);
        }
        let (title, lines) = quest_lines(&ch, &st.content);
        assert_eq!(
            (title.as_str(), lines[0].to_string().as_str()),
            ("QUEST", "no active quest")
        );
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
            (p.stratum, p.pos)
        };
        for h in [10u16, 25, 40] {
            let text = draw_to(&view(&st, pos, k), &st.sidebar, WIDTH, h);
            assert!(
                text.contains("▣ here") && text.contains("▣ inspect"),
                "{text}"
            );
            assert_eq!(text.lines().count(), h as usize);
        }
        let text = draw_to(&view(&st, pos, k), &st.sidebar, WIDTH, 60);
        for needle in [
            "── HERE",
            "── THE DROP",
            "── MAP · street",
            "── LOG",
            "── INSPECT",
            "── LEGEND",
        ] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        assert_eq!(text.lines().count(), 60);
        assert_eq!(
            budget(&[(Section::Here, 3), (Section::Log, 0)], 4),
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
            (p.stratum, p.pos)
        };
        let mut flags = Sidebar::default();
        for s in [Section::Here, Section::Quest, Section::Map] {
            flags.set(s, false);
        }
        // 1 list row + 1 rule + 4 rows: the 70-cell entry needs 3 rows, "short" 1.
        let text = draw_to(&view(&st, pos, k), &flags, WIDTH, 8);
        assert_eq!(text.matches('x').count(), 70, "{text}");
        assert!(text.contains("short"), "{text}");
        // Three rows: the long entry does not fit whole, only "short" shows.
        let text = draw_to(&view(&st, pos, k), &flags, WIDTH, 7);
        assert_eq!(text.matches('x').count(), 0, "{text}");
        assert!(text.contains("short"), "{text}");
    }
}
