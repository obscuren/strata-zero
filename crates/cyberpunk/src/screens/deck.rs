//! The deck: the player's menu over the live map — quests, gear, inventory, workbench, map, skills.

use crate::anim::effects::{glitch_cells, pulse, typewriter};
use crate::anim::{Easing, Tween};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::render::memmap::MemoryMap;
use crate::ui::logo::{self, Logo};
use citygen::rng::hash64;
use citygen::{Pos, StratumKind};
use crossterm::event::{KeyCode, KeyEvent};
use game::{ItemId, ItemKind, Slot};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub const CYAN: Color = Color::Rgb(0, 229, 255);
pub const MAGENTA: Color = Color::Rgb(255, 0, 200);
pub const DIM: Color = Color::Rgb(120, 130, 145);
pub const TEXT: Color = Color::Rgb(205, 212, 222);
const CYAN_RGB: Rgb = Rgb(0, 229, 255);
const MAGENTA_RGB: Rgb = Rgb(255, 0, 200);
const PANEL_BG: Color = Color::Rgb(4, 6, 10);
const PANEL_BG_RGB: Rgb = Rgb(4, 6, 10);
/// How far the washed map is pulled toward the panel colour; the rest shows through.
const GLASS: f32 = 0.45;
const RULE: Color = Color::Rgb(40, 60, 75);
const BAR_BG: Color = Color::Rgb(0, 60, 70);
const REVEAL_TICKS: u64 = 20;
const JITTER_PERIOD: u64 = 90;
const LOGO_MIN_ROWS: u16 = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Quests,
    Gear,
    Inventory,
    Workbench,
    Map,
    Skills,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Quests,
        Tab::Gear,
        Tab::Inventory,
        Tab::Workbench,
        Tab::Map,
        Tab::Skills,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Quests => "QUESTS",
            Tab::Gear => "GEAR",
            Tab::Inventory => "INVENTORY",
            Tab::Workbench => "WORKBENCH",
            Tab::Map => "MAP",
            Tab::Skills => "SKILLS",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Normal,
    Dim,
    Warn,
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Detail {
    Quest(usize),
    Slot(Slot),
    Item(ItemId),
    Recipe(usize),
    Skill(usize),
}

/// One list row and what the detail pane shows for it (`None` = a header).
pub(crate) struct Row {
    pub text: String,
    pub tone: Tone,
    pub detail: Option<Detail>,
}

pub struct Deck {
    pub tab: Tab,
    pub row: usize,
    /// Tick of the last row or tab change; drives the typewriter.
    pub(crate) changed: u64,
    tab_from: usize,
    tab_tween: Tween,
    /// The tile the link line describes: the map's focus when it opened the deck.
    /// Without one the deck falls back to the player.
    focus: Option<(StratumKind, Pos)>,
}

pub(crate) fn kind_icon(kind: ItemKind) -> char {
    match kind {
        ItemKind::Weapon => '/',
        ItemKind::Armor => '▣',
        ItemKind::Cyberware => '◈',
        ItemKind::Deck => '⌗',
        ItemKind::Consumable => '◍',
        ItemKind::Component => '▪',
        ItemKind::Junk => '·',
    }
}

fn tone_style(tone: Tone) -> Style {
    match tone {
        Tone::Normal => Style::default().fg(TEXT),
        Tone::Dim => Style::default().fg(DIM),
        Tone::Warn => Style::default().fg(MAGENTA),
        Tone::Done => Style::default().fg(Color::Rgb(90, 200, 140)),
    }
}

fn head(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into().to_uppercase(),
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    ))
}

fn plain(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(TEXT)))
}

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

fn warn(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(MAGENTA)))
}

/// Heavy top and bottom rules, light sides.
fn frame(buf: &mut Buffer, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let style = Style::default().fg(CYAN);
    for x in area.x..area.right() {
        buf[(x, area.y)].set_char('━').set_style(style);
        buf[(x, area.bottom() - 1)].set_char('━').set_style(style);
    }
    for y in area.y..area.bottom() {
        buf[(area.x, y)].set_char('│').set_style(style);
        buf[(area.right() - 1, y)].set_char('│').set_style(style);
    }
    buf[(area.x, area.y)].set_char('┍').set_style(style);
    buf[(area.right() - 1, area.y)]
        .set_char('┑')
        .set_style(style);
    buf[(area.x, area.bottom() - 1)]
        .set_char('┕')
        .set_style(style);
    buf[(area.right() - 1, area.bottom() - 1)]
        .set_char('┙')
        .set_style(style);
}

/// Tints the panel area toward the panel colour, keeping the washed map as a ghost.
fn glass(buf: &mut Buffer, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            let bg = match cell.bg {
                Color::Rgb(r, g, b) => Rgb(r, g, b).mix(PANEL_BG_RGB, GLASS).to_color(),
                _ => PANEL_BG,
            };
            let fg = match cell.fg {
                Color::Rgb(r, g, b) => Rgb(r, g, b).scale(0.6).to_color(),
                other => other,
            };
            cell.set_bg(bg).set_fg(fg);
        }
    }
}

/// Writes `text` from `x`, stopping before `max_x`, on a solid backing unless the style
/// brings its own background.
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, max_x: u16) {
    let style = if style.bg.is_none() {
        style.bg(PANEL_BG)
    } else {
        style
    };
    for (cx, ch) in (x..max_x).zip(text.chars()) {
        buf[(cx, y)].set_char(ch).set_style(style);
    }
}

/// Renders lines into `area`, backing only the cells that carry text.
fn put_paragraph(buf: &mut Buffer, area: Rect, lines: Vec<Line<'static>>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut tmp = Buffer::empty(area);
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(area, &mut tmp);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let src = &tmp[(x, y)];
            if src.symbol() != " " {
                let style = src.style().bg(PANEL_BG);
                buf[(x, y)].set_symbol(src.symbol()).set_style(style);
            }
        }
    }
}

/// Reveals `lines` progressively: `p` of all characters are shown, then a cursor.
fn reveal(lines: &[Line<'static>], p: f32, cursor_on: bool) -> Vec<Line<'static>> {
    if p >= 1.0 {
        return lines.to_vec();
    }
    let total: usize = lines.iter().map(|l| l.width()).sum();
    let mut budget = ((total as f32) * p.clamp(0.0, 1.0)).floor() as usize;
    let cursor = || Span::styled("▮", Style::default().fg(MAGENTA));
    let mut out = Vec::new();
    for line in lines {
        let w = line.width();
        if w <= budget {
            out.push(line.clone());
            budget -= w;
            continue;
        }
        let mut shown: Vec<Span<'static>> = Vec::new();
        let mut left = budget;
        for span in &line.spans {
            let text = span.content.to_string();
            let n = text.chars().count();
            if n <= left {
                shown.push(span.clone());
                left -= n;
            } else {
                let frac = if n == 0 { 1.0 } else { left as f32 / n as f32 };
                shown.push(Span::styled(
                    typewriter(&text, frac).to_string(),
                    span.style,
                ));
                break;
            }
        }
        if cursor_on {
            shown.push(cursor());
        }
        out.push(Line::from(shown));
        return out;
    }
    if cursor_on {
        out.push(Line::from(cursor()));
    }
    out
}

impl Deck {
    pub fn new(tick: u64) -> Self {
        Deck {
            tab: Tab::Quests,
            row: 0,
            changed: tick,
            tab_from: 0,
            tab_tween: Tween::new(tick, 1, Easing::OutCubic),
            focus: None,
        }
    }

    /// Links the deck to a tile: the cursor under the free camera, the player when walking.
    pub fn focus(mut self, stratum: StratumKind, pos: Pos) -> Self {
        self.focus = Some((stratum, pos));
        self
    }

    fn set_tab(&mut self, tab: Tab, st: &AppState) {
        if tab == self.tab {
            return;
        }
        self.tab_from = self.tab.index();
        self.tab = tab;
        self.row = Self::first_selectable(&self.rows(st));
        self.changed = st.tick;
        self.tab_tween = Tween::new(st.tick, 6, Easing::OutCubic);
    }

    fn shift_tab(&mut self, delta: i32, st: &AppState) {
        let n = Tab::ALL.len() as i32;
        let i = (self.tab.index() as i32 + delta).rem_euclid(n) as usize;
        self.set_tab(Tab::ALL[i], st);
    }

    /// Moves the selection by `delta`, skipping header rows; clamps at the ends.
    fn move_row(&mut self, rows: &[Row], delta: i32, tick: u64) {
        if rows.is_empty() {
            self.row = 0;
            return;
        }
        let mut i = self.row as i32;
        loop {
            i += delta;
            if i < 0 || i >= rows.len() as i32 {
                return;
            }
            if rows[i as usize].detail.is_some() {
                self.row = i as usize;
                self.changed = tick;
                return;
            }
        }
    }

    fn first_selectable(rows: &[Row]) -> usize {
        rows.iter().position(|r| r.detail.is_some()).unwrap_or(0)
    }

    pub(crate) fn rows(&self, st: &AppState) -> Vec<Row> {
        match self.tab {
            Tab::Quests => Self::quest_rows(st),
            Tab::Gear => Self::gear_rows(st),
            Tab::Inventory => Self::inventory_rows(st),
            Tab::Workbench => Self::workbench_rows(st),
            Tab::Map => Vec::new(),
            Tab::Skills => Self::skill_rows(st),
        }
    }

    fn quest_rows(st: &AppState) -> Vec<Row> {
        let mut active = Vec::new();
        let mut done = Vec::new();
        for (i, log) in st.character.quests.iter().enumerate() {
            let Some(q) = st.content.quest(&log.quest) else {
                continue;
            };
            let finished = log.is_done();
            let row = Row {
                text: format!("{} {}", if finished { "✓" } else { " " }, q.title),
                tone: if finished { Tone::Done } else { Tone::Normal },
                detail: Some(Detail::Quest(i)),
            };
            if finished {
                done.push(row);
            } else {
                active.push(row);
            }
        }
        active.extend(done);
        active
    }

    fn gear_rows(st: &AppState) -> Vec<Row> {
        Slot::ALL
            .iter()
            .map(|slot| {
                let name = st
                    .character
                    .equipped(*slot)
                    .and_then(|id| st.content.item(id))
                    .map(|i| i.name.clone());
                Row {
                    tone: if name.is_some() {
                        Tone::Normal
                    } else {
                        Tone::Dim
                    },
                    text: format!(
                        "{:<10} {}",
                        slot.label().to_uppercase(),
                        name.unwrap_or_else(|| "— empty —".to_string())
                    ),
                    detail: Some(Detail::Slot(*slot)),
                }
            })
            .collect()
    }

    fn inventory_rows(st: &AppState) -> Vec<Row> {
        let mut rows = Vec::new();
        for kind in ItemKind::ALL {
            let stacks: Vec<_> = st
                .character
                .inventory
                .iter()
                .filter(|s| st.content.item(&s.item).is_some_and(|i| i.kind == kind))
                .collect();
            if stacks.is_empty() {
                continue;
            }
            rows.push(Row {
                text: format!("── {} ──", kind.label()),
                tone: Tone::Dim,
                detail: None,
            });
            for s in stacks {
                let item = st.content.item(&s.item).expect("filtered above");
                rows.push(Row {
                    text: format!("{} {:>2}× {}", kind_icon(kind), s.qty, item.name),
                    tone: Tone::Normal,
                    detail: Some(Detail::Item(s.item.clone())),
                });
            }
        }
        rows
    }

    fn workbench_rows(st: &AppState) -> Vec<Row> {
        st.content
            .recipes
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let ok = st.character.can_craft(&st.content, r);
                Row {
                    text: format!("{} {}", if ok { "▪" } else { "▫" }, r.name),
                    tone: if ok { Tone::Normal } else { Tone::Warn },
                    detail: Some(Detail::Recipe(i)),
                }
            })
            .collect()
    }

    fn skill_rows(st: &AppState) -> Vec<Row> {
        st.content
            .skills
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let have = st.character.skill_level(&s.id);
                let lvl = have.min(5) as usize;
                let bar = format!("{}{}", "▰".repeat(lvl), "▱".repeat(5 - lvl));
                Row {
                    text: format!("{:<12} {bar} {have}", s.name),
                    tone: Tone::Normal,
                    detail: Some(Detail::Skill(i)),
                }
            })
            .collect()
    }

    fn item_lines(st: &AppState, id: &ItemId) -> Vec<Line<'static>> {
        let Some(item) = st.content.item(id) else {
            return vec![warn(format!("unknown item {id}"))];
        };
        let mut out = vec![head(&item.name)];
        let slot = item
            .slot
            .map(|s| format!(" · {}", s.label()))
            .unwrap_or_default();
        out.push(dim(format!("{}{slot}", item.kind.label())));
        out.push(dim(format!("{:.1} kg · ¤ {}", item.weight, item.value)));
        if st.character.is_equipped(id) {
            out.push(Line::from(Span::styled(
                "equipped",
                Style::default().fg(CYAN),
            )));
        }
        out.push(plain(""));
        out.push(plain(item.desc.clone()));
        out
    }

    pub(crate) fn detail(&self, st: &AppState, rows: &[Row]) -> Vec<Line<'static>> {
        let Some(row) = rows.get(self.row) else {
            return vec![dim("nothing here yet")];
        };
        match &row.detail {
            None => Vec::new(),
            Some(Detail::Quest(i)) => {
                let log = &st.character.quests[*i];
                let Some(q) = st.content.quest(&log.quest) else {
                    return Vec::new();
                };
                let mut out = vec![head(&q.title)];
                out.push(dim(format!("from {} · {}", q.giver, q.location)));
                out.push(plain(""));
                out.push(plain(q.desc.clone()));
                out.push(plain(""));
                for (j, text) in q.objectives.iter().enumerate() {
                    let done = log.done.get(j).copied().unwrap_or(false);
                    out.push(if done {
                        Line::from(Span::styled(format!("☑ {text}"), tone_style(Tone::Done)))
                    } else {
                        plain(format!("☐ {text}"))
                    });
                }
                out
            }
            Some(Detail::Slot(slot)) => match st.character.equipped(*slot) {
                Some(id) => Self::item_lines(st, id),
                None => vec![
                    head("— empty —"),
                    dim(format!("nothing in the {} slot", slot.label())),
                ],
            },
            Some(Detail::Item(id)) => Self::item_lines(st, id),
            Some(Detail::Recipe(i)) => {
                let r = &st.content.recipes[*i];
                let skill = st
                    .content
                    .skill(&r.skill)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                let have = st.character.skill_level(&r.skill);
                let mut out = vec![head(&r.name)];
                let need = format!("needs {skill} {}  (you: {have})", r.level);
                out.push(if have >= r.level {
                    dim(need)
                } else {
                    warn(need)
                });
                out.push(plain(""));
                for (id, n) in &r.inputs {
                    let name = st
                        .content
                        .item(id)
                        .map(|i| i.name.clone())
                        .unwrap_or_else(|| id.to_string());
                    let got = st.character.count(id);
                    let line = format!("{got}/{n}  {name}");
                    out.push(if got >= *n { plain(line) } else { warn(line) });
                }
                let out_name = st
                    .content
                    .item(&r.output.0)
                    .map(|i| i.name.clone())
                    .unwrap_or_default();
                out.push(Line::from(Span::styled(
                    format!("→ {}× {out_name}", r.output.1),
                    Style::default().fg(CYAN),
                )));
                out.push(plain(""));
                out.push(plain(r.desc.clone()));
                out
            }
            Some(Detail::Skill(i)) => {
                let s = &st.content.skills[*i];
                let mut out = vec![head(&s.name)];
                out.push(dim(format!("stat · {}", s.stat.label())));
                out.push(plain(""));
                out.push(plain(s.desc.clone()));
                out.push(plain(""));
                out.push(dim("talents"));
                for t in st.content.talents_for(&s.id) {
                    if st.character.has_talent(&t.id) {
                        out.push(Line::from(Span::styled(
                            format!("◈ {}", t.name),
                            Style::default().fg(CYAN),
                        )));
                        out.push(dim(format!("   {}", t.desc)));
                    } else {
                        out.push(dim(format!("  {}  (needs {})", t.name, t.level)));
                    }
                }
                out
            }
        }
    }

    fn link_line(&self, st: &AppState) -> String {
        let at = self
            .focus
            .or_else(|| st.player.as_ref().map(|p| (p.stratum, p.pos)));
        let Some((stratum, pos)) = at else {
            return "▍ LINK · no fix".to_string();
        };
        // Physical strata carry the district on every tile; the Net reads the ground below.
        let district = st
            .city
            .as_ref()
            .and_then(|c| {
                c.tile(stratum, pos)
                    .and_then(|t| t.district)
                    .or_else(|| c.tile(StratumKind::Street, pos).and_then(|t| t.district))
                    .and_then(|d| c.district_opt(d))
                    .map(|d| d.name.clone())
            })
            .unwrap_or_else(|| "no fix".to_string());
        format!("▍ LINK · {district} · {}", stratum.label())
    }

    fn draw_map(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        match (&st.player, &st.city) {
            (Some(p), Some(city)) => {
                let inner = Rect::new(
                    body.x + 1,
                    body.y + 1,
                    body.width.saturating_sub(2),
                    body.height.saturating_sub(1),
                );
                MemoryMap {
                    city,
                    stratum: p.stratum,
                    vis: p.vis(),
                    player: Some(p.pos),
                    style: &st.style,
                }
                .render(inner, buf);
                put(
                    buf,
                    body.x + 1,
                    body.y,
                    &format!(" {} · what you remember ", p.stratum.label()),
                    Style::default().fg(DIM).bg(PANEL_BG),
                    body.right(),
                );
            }
            _ => put(
                buf,
                body.x + 1,
                body.y + 1,
                "no signal — walk the city first",
                Style::default().fg(DIM),
                body.right(),
            ),
        }
    }
}

impl Screen for Deck {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            let rows = self.rows(st);
            if self.row >= rows.len() || rows.get(self.row).is_some_and(|r| r.detail.is_none()) {
                self.row = Self::first_selectable(&rows);
            }
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::Glitch,
                    };
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.shift_tab(-1, st),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.shift_tab(1, st),
                KeyCode::Char(c @ '1'..='6') => {
                    let i = c as usize - '1' as usize;
                    self.set_tab(Tab::ALL[i], st);
                }
                KeyCode::Up | KeyCode::Char('k') => self.move_row(&rows, -1, st.tick),
                KeyCode::Down | KeyCode::Char('j') => self.move_row(&rows, 1, st.tick),
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let tick = st.tick;
        let buf = f.buffer_mut();
        // The washed map stays visible as a ghost under the chrome; text gets a solid backing.
        glass(buf, area);
        frame(buf, area);
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        let right = inner.right();
        let mut y = inner.y;

        // Title: block-font handle when tall enough, else a caption; readout on the right.
        let handle = st.character.handle.to_uppercase();
        let readout = format!(
            "¤ {} · hp {}/{} · {:.1} kg",
            st.character.credits,
            st.character.hp,
            st.character.hp_max,
            st.character.weight(&st.content)
        );
        if area.height >= LOGO_MIN_ROWS {
            let w = logo::width(&handle).min(inner.width);
            Logo {
                text: &handle,
                tick,
                accent: MAGENTA_RGB,
            }
            .render(Rect::new(inner.x + 1, y, w, logo::ROWS), buf);
            y += logo::ROWS;
        } else {
            put(
                buf,
                inner.x + 1,
                y,
                &handle,
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                right,
            );
        }
        let rx = right.saturating_sub(readout.chars().count() as u16 + 1);
        put(buf, rx, inner.y, &readout, Style::default().fg(DIM), right);
        y += 1;

        // Link line.
        let glow = 0.6 + 0.4 * pulse(tick, 40);
        put(
            buf,
            inner.x + 1,
            y,
            &self.link_line(st),
            Style::default().fg(CYAN_RGB.scale(glow).to_color()),
            right,
        );
        y += 1;

        // Tab bar.
        let mut x = inner.x + 1;
        let mut starts = [0u16; 6];
        for (i, tab) in Tab::ALL.iter().enumerate() {
            let active = *tab == self.tab;
            let label = if active {
                format!("▌{}▐", tab.label())
            } else {
                format!(" {} ", tab.label())
            };
            starts[i] = x;
            let style = if active {
                Style::default()
                    .fg(CYAN_RGB.scale(0.7 + 0.3 * pulse(tick, 30)).to_color())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(DIM)
            };
            put(buf, x, y, &label, style, right);
            x += label.chars().count() as u16 + 2;
        }
        y += 1;
        // Rule with the sliding magenta segment under the active tab.
        for cx in inner.x..right {
            buf[(cx, y)].set_char('─').set_fg(RULE);
        }
        let p = self.tab_tween.progress(tick);
        let from = starts[self.tab_from] as f32;
        let to = starts[self.tab.index()] as f32;
        let seg_x = (from + (to - from) * p).round() as u16;
        let seg_w = self.tab.label().chars().count() as u16 + 2;
        for cx in seg_x..(seg_x + seg_w).min(right) {
            buf[(cx, y)].set_char('━').set_fg(MAGENTA);
        }
        y += 1;

        // Body.
        let hint_y = inner.bottom().saturating_sub(1);
        let body = Rect::new(inner.x, y, inner.width, hint_y.saturating_sub(y));
        if self.tab == Tab::Map {
            self.draw_map(st, body, buf);
        } else {
            let list_w = (body.width * 2 / 5)
                .max(28)
                .min(body.width.saturating_sub(10));
            // The inventory keeps two rows at the bottom: a rule and the weight/value footer.
            let reserved = if self.tab == Tab::Inventory { 2 } else { 0 };
            let list = Rect::new(body.x, body.y, list_w, body.height.saturating_sub(reserved));
            let detail = Rect::new(
                body.x + list_w + 1,
                body.y,
                body.width.saturating_sub(list_w + 1),
                body.height,
            );
            for cy in body.y..body.bottom() {
                buf[(body.x + list_w, cy)].set_char('║').set_fg(RULE);
            }
            let rows = self.rows(st);
            let half = list.height.saturating_sub(1) as usize / 2;
            let max_first = rows.len().saturating_sub(list.height as usize);
            let first = self.row.saturating_sub(half).min(max_first);
            for (i, row) in rows
                .iter()
                .enumerate()
                .skip(first)
                .take(list.height as usize)
            {
                let ry = list.y + (i - first) as u16;
                let selected = i == self.row && row.detail.is_some();
                let mut style = tone_style(row.tone);
                if selected {
                    style = style
                        .bg(BAR_BG)
                        .fg(CYAN_RGB.scale(0.7 + 0.3 * pulse(tick, 30)).to_color())
                        .add_modifier(Modifier::BOLD);
                    for cx in list.x..list.right() {
                        buf[(cx, ry)].set_bg(BAR_BG);
                    }
                }
                let text = format!("{}{}", if selected { "▸ " } else { "  " }, row.text);
                put(buf, list.x + 1, ry, &text, style, list.right());
            }
            if self.tab == Tab::Inventory {
                let footer = format!(
                    "{:.1} kg · ¤ {}",
                    st.character.weight(&st.content),
                    st.character.value(&st.content)
                );
                for cx in list.x + 1..list.right() {
                    buf[(cx, list.bottom())].set_char('─').set_fg(RULE);
                }
                put(
                    buf,
                    list.x + 1,
                    list.bottom() + 1,
                    &footer,
                    Style::default().fg(DIM),
                    list.right(),
                );
            }
            let lines = self.detail(st, &rows);
            let p = (tick.saturating_sub(self.changed)) as f32 / REVEAL_TICKS as f32;
            let shown = reveal(&lines, p, (tick / 4).is_multiple_of(2));
            put_paragraph(
                buf,
                Rect::new(
                    detail.x + 1,
                    detail.y,
                    detail.width.saturating_sub(2),
                    detail.height,
                ),
                shown,
            );
        }

        // Hints.
        put(
            buf,
            inner.x + 1,
            hint_y,
            "←→ tabs · ↑↓ select · 1-6 jump · esc close",
            Style::default().fg(DIM),
            right,
        );

        // Signal jitter: a three-tick burst over one row every JITTER_PERIOD ticks.
        if tick % JITTER_PERIOD < 3 {
            let row = area.y + (hash64(tick / JITTER_PERIOD, 7) % area.height.max(1) as u64) as u16;
            let p = (tick % JITTER_PERIOD) as f32 / 3.0;
            for (gx, gy, ch) in glitch_cells(Rect::new(area.x, row, area.width, 1), tick, p) {
                let tint = if hash64(gx as u64, gy as u64).is_multiple_of(2) {
                    CYAN
                } else {
                    MAGENTA
                };
                buf[(gx, gy)].set_char(ch).set_fg(tint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    fn draw(d: &Deck, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn deck_tabs_and_rows_render_from_content() {
        let mut st = state();
        let mut d = Deck::new(0);
        st.tick = 40;
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("▌QUESTS▐"), "{text}");
        assert!(
            text.contains("VEX") && text.contains("¤ 320") && text.contains("hp 40/40"),
            "{text}"
        );
        assert!(
            text.contains("The drop") && text.contains("✓ First jack"),
            "{text}"
        );
        assert!(text.contains("LINK"), "{text}");
        assert!(
            draw(&d, &st, 120, 40).contains('█'),
            "tall decks set the handle in block font"
        );

        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        assert_eq!(d.tab, Tab::Gear);
        st.tick += 30;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("▌GEAR▐") && text.contains("PRIMARY") && text.contains("Ratchet SMG"),
            "{text}"
        );
        assert!(text.contains("— empty —"), "{text}");
        d.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(d.row, 1, "eyes");
        st.tick += 30;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("NIGHTGLASS OPTICS") && text.contains("cyberware · eyes"),
            "{text}"
        );

        d.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!(d.tab, Tab::Inventory);
        st.tick += 30;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("── component ──") && text.contains("3× Copper coil"),
            "{text}"
        );
        assert!(text.contains("kg") && text.contains("¤"), "{text}");
        // The first row is an item, never a header; headers are skipped when moving.
        let rows = d.rows(&st);
        assert!(rows[d.row].detail.is_some());
        let before = d.row;
        d.update(&mut st, &[key(KeyCode::Down)]);
        assert!(rows[d.row].detail.is_some() && d.row > before);

        d.update(
            &mut st,
            &[key(KeyCode::Left), key(KeyCode::Left), key(KeyCode::Left)],
        );
        assert_eq!(d.tab, Tab::Skills, "left wraps around");
        d.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(d.tab, Tab::Quests, "tab wraps forward");
        d.update(&mut st, &[key(KeyCode::BackTab)]);
        assert_eq!(d.tab, Tab::Skills);
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop {
                fx: TransitionKind::Glitch
            }
        ));
    }

    #[test]
    fn deck_fits_the_minimum_terminal() {
        let mut st = state();
        st.tick = 40;
        let d = Deck::new(0);
        let text = draw(&d, &st, 100, 30);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].starts_with('┍') && lines[0].ends_with('┑'),
            "{:?}",
            lines[0]
        );
        assert!(
            lines[29].starts_with('┕') && lines[29].ends_with('┙'),
            "{:?}",
            lines[29]
        );
        assert!(
            text.contains("▌QUESTS▐") && text.contains("SKILLS"),
            "{text}"
        );
        assert!(text.contains("esc close"), "{text}");
        assert!(text.contains("║"), "no pane divider: {text}");
        for (i, l) in lines.iter().enumerate().take(29).skip(1) {
            assert!(l.starts_with('│') && l.ends_with('│'), "row {i}: {l:?}");
        }
    }

    #[test]
    fn deck_glyphs_are_single_width() {
        use unicode_width::UnicodeWidthChar;
        for c in [
            '▰', '▱', '☑', '☐', '✓', '▮', '◍', '┍', '┑', '┕', '┙', '▌', '▐', '║', '◈', '⌗', '▣',
            '▪', '▫',
        ] {
            assert_eq!(c.width(), Some(1), "{c:?}");
        }
    }

    #[test]
    fn workbench_and_skills_show_what_you_can_and_cannot_do() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('4'))]);
        st.tick = 60;
        let rows = d.rows(&st);
        let shiv = rows.iter().find(|r| r.text.contains("Wire shiv")).unwrap();
        let optics = rows
            .iter()
            .find(|r| r.text.contains("Optics tune"))
            .unwrap();
        assert_eq!(shiv.tone, Tone::Normal);
        assert_eq!(optics.tone, Tone::Warn);
        d.row = rows
            .iter()
            .position(|r| r.text.contains("Optics tune"))
            .unwrap();
        d.changed = 0;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("needs Electronics 4") && text.contains("you: 2"),
            "{text}"
        );
        assert!(
            text.contains("1/1") || text.contains("0/1"),
            "have/need: {text}"
        );

        d.update(&mut st, &[key(KeyCode::Char('6'))]);
        d.changed = 0;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("Hacking") && text.contains("▰▰▰▱▱ 3"),
            "{text}"
        );
        let rows = d.rows(&st);
        d.row = rows
            .iter()
            .position(|r| r.text.contains("Hacking"))
            .unwrap();
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("◈ Ice pick") && text.contains("Deep dive") && text.contains("needs 4"),
            "{text}"
        );
    }

    #[test]
    fn map_tab_shows_memory_or_no_signal() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('5'))]);
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("no signal"), "{text}");
        let city = st.city.as_ref().unwrap();
        st.player = Some(crate::walker::Player::spawn(city, 1));
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains('@'), "{text}");
        assert!(!text.contains("no signal"));
    }

    #[test]
    fn deck_is_glass_over_the_washed_map() {
        let mut st = state();
        st.tick = 40;
        let d = Deck::new(0);
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| {
            let area = f.area();
            let buf = f.buffer_mut();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    buf[(x, y)]
                        .set_char('x')
                        .set_fg(Color::Rgb(90, 90, 90))
                        .set_bg(Color::Rgb(40, 40, 40));
                }
            }
            d.draw(&st, f);
        })
        .unwrap();
        let buf = t.backend().buffer();
        // Somewhere inside the body, away from text, the map still shows through.
        let ghosts = (6..33u16)
            .flat_map(|y| (60..118u16).map(move |x| (x, y)))
            .filter(|(x, y)| buf[(*x, *y)].symbol() == "x")
            .count();
        assert!(ghosts > 100, "ghosts {ghosts}: the deck is opaque");
        let (gx, gy) = (110u16, 30u16);
        assert_eq!(buf[(gx, gy)].symbol(), "x");
        assert_ne!(
            buf[(gx, gy)].bg,
            Color::Rgb(40, 40, 40),
            "bg is tinted, not raw"
        );
        assert_ne!(buf[(gx, gy)].bg, PANEL_BG, "bg is tinted, not solid");
        // Text rows sit on the solid panel colour so they stay legible.
        let row = buffer_text(buf);
        assert!(row.contains("The drop"));
    }

    /// Forty loose coils: more inventory rows than any pane has.
    fn stuff(st: &mut AppState) {
        use game::Stack;
        st.character.inventory.extend((0..40).map(|_| Stack {
            item: ItemId::new("copper_coil"),
            qty: 1,
        }));
    }

    #[test]
    fn map_tab_ignores_row_keys() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(
            &mut st,
            &[
                key(KeyCode::Char('5')),
                key(KeyCode::Down),
                key(KeyCode::Char('j')),
                key(KeyCode::Up),
            ],
        );
        assert_eq!((d.tab, d.row), (Tab::Map, 0));
        assert!(draw(&d, &st, 100, 30).contains("no signal"));
    }

    #[test]
    fn skills_row_shows_the_real_level() {
        let mut st = state();
        for (id, lvl) in &mut st.character.skills {
            if id.0 == "hacking" {
                *lvl = 7;
            }
        }
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('6'))]);
        let rows = d.rows(&st);
        let hacking = rows.iter().find(|r| r.text.contains("Hacking")).unwrap();
        assert!(hacking.text.contains("▰▰▰▰▰ 7"), "{:?}", hacking.text);
    }

    #[test]
    fn inventory_footer_keeps_its_own_row() {
        let mut st = state();
        stuff(&mut st);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        st.tick = 60;
        let pane = |text: &str| -> Vec<String> {
            text.lines()
                .map(|l| l.split('║').next().unwrap_or("").to_string())
                .collect()
        };
        let left = pane(&draw(&d, &st, 100, 30));
        let footer = left.iter().find(|l| l.contains("kg · ¤")).expect("footer");
        assert!(
            !footer.contains('×'),
            "footer shares a row with a stack: {footer:?}"
        );
        d.update(&mut st, &[key(KeyCode::Down); 60]);
        let left = pane(&draw(&d, &st, 100, 30));
        let sel = left
            .iter()
            .find(|l| l.contains("▸ "))
            .expect("the selected row is visible at the end of the list");
        assert!(!sel.contains("kg"), "{sel:?}");
        assert!(
            left.iter().any(|l| l.contains("kg · ¤")),
            "footer still there"
        );
    }

    #[test]
    fn list_scroll_fills_the_pane_at_the_end() {
        let mut st = state();
        stuff(&mut st);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        d.update(&mut st, &[key(KeyCode::Down); 60]);
        st.tick = 60;
        let text = draw(&d, &st, 100, 30);
        let visible = text.lines().filter(|l| l.contains("× ")).count();
        assert!(
            visible >= 18,
            "only {visible} stacks visible at the end:\n{text}"
        );
    }

    #[test]
    fn memory_map_caption_sits_above_the_map() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('5'))]);
        let mut p = crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1);
        p.pos = citygen::Pos::new(0, 0);
        st.player = Some(p);
        let text = draw(&d, &st, 120, 35);
        let lines: Vec<&str> = text.lines().collect();
        let cap = lines
            .iter()
            .position(|l| l.contains("what you remember"))
            .unwrap();
        assert!(!lines[cap].contains('@'), "caption row: {:?}", lines[cap]);
        assert!(lines[cap + 1].contains('@'), "{text}");
    }

    #[test]
    fn link_line_follows_the_focus_then_the_player() {
        use citygen::Pos;
        let mut st = state();
        st.tick = 60;
        let (pos, name) = {
            let city = st.city.as_ref().unwrap();
            let street = StratumKind::Street;
            (0..city.size.h as i32)
                .flat_map(|y| (0..city.size.w as i32).map(move |x| Pos::new(x, y)))
                .find_map(|p| {
                    let d = city.tile(street, p)?.district?;
                    Some((p, city.district(d).name.clone()))
                })
                .expect("a tile with a district")
        };
        assert!(draw(&Deck::new(0), &st, 120, 35).contains("LINK · no fix"));
        let text = draw(&Deck::new(0).focus(StratumKind::Street, pos), &st, 120, 35);
        assert!(text.contains(&format!("LINK · {name} · Street")), "{text}");
        // A Net tile carries no district: the ground below names the link.
        let text = draw(&Deck::new(0).focus(StratumKind::Net, pos), &st, 120, 35);
        assert!(
            text.contains(&format!("LINK · {name} · {}", StratumKind::Net.label())),
            "{text}"
        );
        let mut p = crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1);
        p.pos = pos;
        st.player = Some(p);
        let text = draw(&Deck::new(0), &st, 120, 35);
        assert!(text.contains(&format!("LINK · {name} · Street")), "{text}");
    }
}
