//! The deck: the player's menu over the live map — quests, gear, inventory, workbench,
//! map, skills, abilities.

use crate::anim::effects::{glitch_cells, pulse, typewriter};
use crate::anim::{Easing, Tween};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::messages::Kind;
use crate::render::glyphs::kind_icon;
use crate::render::memmap::MemoryMap;
use crate::screens::popup::{Choice, Pick, Popup};
use crate::ui::chrome::{
    BAR_BG, CYAN, CYAN_RGB, DIM, MAGENTA, MAGENTA_RGB, PANEL_BG, RULE, TEXT, dim, frame, glass,
    head, plain, put, put_paragraph, reveal, warn,
};
use crate::ui::logo::{self, Logo};
use citygen::rng::hash64;
use citygen::{Pos, StratumKind};
use crossterm::event::{KeyCode, KeyEvent};
use game::{AbilityKind, Action, Active, ItemId, ItemKind, Perk, Slot, Source};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::cell::Cell;

const REVEAL_TICKS: u64 = 20;
const JITTER_PERIOD: u64 = 90;
const LOGO_MIN_ROWS: u16 = 36;
/// Hotkey letters for picker rows; h j k l move, q closes, u and i are verbs.
const LETTERS: &str = "abcdefgmnoprstvwxyz";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Quests,
    Gear,
    Inventory,
    Workbench,
    Map,
    Skills,
    Abilities,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Quests,
        Tab::Gear,
        Tab::Inventory,
        Tab::Workbench,
        Tab::Map,
        Tab::Skills,
        Tab::Abilities,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Quests => "QUESTS",
            Tab::Gear => "GEAR",
            Tab::Inventory => "INVENTORY",
            Tab::Workbench => "WORKBENCH",
            Tab::Map => "MAP",
            Tab::Skills => "SKILLS",
            Tab::Abilities => "ABILITIES",
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
    /// Index into `Character::abilities`.
    Ability(usize),
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
    /// The list pane and its first visible row at the last draw, to anchor popups.
    last_list: Cell<(Rect, usize)>,
}

fn tone_style(tone: Tone) -> Style {
    match tone {
        Tone::Normal => Style::default().fg(TEXT),
        Tone::Dim => Style::default().fg(DIM),
        Tone::Warn => Style::default().fg(MAGENTA),
        Tone::Done => Style::default().fg(Color::Rgb(90, 200, 140)),
    }
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
            last_list: Cell::new((Rect::new(0, 0, 0, 0), 0)),
        }
    }

    /// The popup for the selected row: a picker for a gear slot, verbs for an item.
    /// Rows with nothing to do give `None`; an empty slot nothing fits says so in the strip.
    pub(crate) fn interact(&self, st: &mut AppState) -> Option<Popup> {
        let rows = self.rows(st);
        let row = rows.get(self.row)?;
        let (list, first) = self.last_list.get();
        let anchor = (
            list.right() + 2,
            list.y + self.row.saturating_sub(first) as u16,
        );
        match &row.detail {
            Some(Detail::Slot(slot)) => {
                let mut choices = Vec::new();
                let mut letters = LETTERS.chars();
                for s in &st.character.inventory {
                    let Some(item) = st.content.item(&s.item) else {
                        continue;
                    };
                    if item.slot != Some(*slot) {
                        continue;
                    }
                    let Some(k) = letters.next() else { break };
                    choices.push(Choice::new(
                        k,
                        item.name.clone(),
                        Pick::Act(Action::Equip(s.item.clone())),
                    ));
                }
                if let Some(id) = st.character.equipped(*slot) {
                    choices.push(Choice::new(
                        'u',
                        "unequip",
                        Pick::Act(Action::Unequip(*slot)),
                    ));
                    choices.push(Choice::new(
                        'i',
                        "inspect",
                        Pick::Card(Self::item_card(st, id)),
                    ));
                }
                if choices.is_empty() {
                    st.warn(format!("nothing in the pack fits {}", slot.label()));
                    return None;
                }
                Some(
                    Popup::verbs(&slot.label().to_uppercase(), choices, st.tick)
                        .anchored(anchor.0, anchor.1),
                )
            }
            Some(Detail::Item(id)) => {
                let item = st.content.item(id)?;
                let qty = st.character.count(id);
                let mut choices = vec![Choice::new(
                    'i',
                    "inspect",
                    Pick::Card(Self::item_card(st, id)),
                )];
                if item.slot.is_some() {
                    choices.push(Choice::new(
                        'e',
                        "equip",
                        Pick::Act(Action::Equip(id.clone())),
                    ));
                }
                if item.on_use.is_some() {
                    choices.push(Choice::new('u', "use", Pick::Act(Action::Use(id.clone()))));
                }
                choices.push(Choice::new(
                    'd',
                    "drop one",
                    Pick::Act(Action::Drop {
                        item: id.clone(),
                        qty: 1,
                    }),
                ));
                if qty > 1 {
                    choices.push(Choice::new(
                        'D',
                        "drop all",
                        Pick::Act(Action::Drop {
                            item: id.clone(),
                            qty,
                        }),
                    ));
                }
                Some(
                    Popup::verbs(&item.name.to_uppercase(), choices, st.tick)
                        .anchored(anchor.0, anchor.1),
                )
            }
            _ => None,
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
        // Wrap at both ends, landing on the next row that has a detail.
        let n = rows.len() as i32;
        let mut i = self.row as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
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
            Tab::Abilities => Self::ability_rows(st),
        }
    }

    fn ability_rows(st: &AppState) -> Vec<Row> {
        st.character
            .abilities(&st.content)
            .iter()
            .enumerate()
            .filter_map(|(i, (id, _))| {
                let a = st.content.ability(id)?;
                let (icon, state, tone) = match a.kind {
                    AbilityKind::Passive => ('·', "passive".to_string(), Tone::Dim),
                    AbilityKind::Active(_) => {
                        match st.character.cooling(&st.content, id, st.tick) {
                            Some(left) => (
                                '◈',
                                format!("active · cooling {}s", left.div_ceil(30)),
                                Tone::Warn,
                            ),
                            None => ('◈', "active · ready".to_string(), Tone::Normal),
                        }
                    }
                };
                Some(Row {
                    text: format!("{icon} {:<12} {state}", a.name),
                    tone,
                    detail: Some(Detail::Ability(i)),
                })
            })
            .collect()
    }

    /// What an item does while worn: one line per perk, one per granted ability.
    pub(crate) fn perk_lines(st: &AppState, item: &game::Item) -> Vec<Line<'static>> {
        let cyan = |t: String| Line::from(Span::styled(t, Style::default().fg(CYAN)));
        let mut out = Vec::new();
        for p in &item.perks {
            out.push(cyan(match p {
                Perk::Stat(s, n) => format!("{n:+} {}", s.label()),
                Perk::Skill(id, n) => format!(
                    "{n:+} {}",
                    st.content
                        .skill(id)
                        .map(|s| s.name.to_lowercase())
                        .unwrap_or_else(|| id.to_string())
                ),
                Perk::Sight(n) => format!("{n:+} sight"),
                Perk::Carry(kg) => format!("{kg:+.1} kg carry"),
                Perk::HpMax(n) => format!("{n:+} max hp"),
            }));
        }
        for id in &item.grants {
            let name = st
                .content
                .ability(id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| id.to_string());
            out.push(cyan(format!("grants {name}")));
        }
        out
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
                let have = st.character.skill_level(&st.content, &s.id);
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

    /// Everything about one item: the detail pane and the inspect card.
    pub(crate) fn item_card(st: &AppState, id: &ItemId) -> Vec<Line<'static>> {
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
        out.extend(Self::perk_lines(st, item));
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
                Some(id) => Self::item_card(st, id),
                None => vec![
                    head("— empty —"),
                    dim(format!("nothing in the {} slot", slot.label())),
                ],
            },
            Some(Detail::Item(id)) => Self::item_card(st, id),
            Some(Detail::Recipe(i)) => {
                let r = &st.content.recipes[*i];
                let skill = st
                    .content
                    .skill(&r.skill)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                let have = st.character.skill_level(&st.content, &r.skill);
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
                let mut bonus: Vec<String> = Vec::new();
                for slot in Slot::ALL {
                    let Some(item) = st
                        .character
                        .equipped(slot)
                        .and_then(|id| st.content.item(id))
                    else {
                        continue;
                    };
                    for p in &item.perks {
                        if let Perk::Skill(sid, n) = p
                            && sid == &s.id
                        {
                            bonus.push(format!("{n:+} {}", item.name));
                        }
                    }
                }
                if !bonus.is_empty() {
                    out.push(dim(format!(
                        "base {} · {}",
                        st.character.skill_base(&s.id),
                        bonus.join(" · ")
                    )));
                }
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
            Some(Detail::Ability(i)) => {
                let abilities = st.character.abilities(&st.content);
                let Some((id, source)) = abilities.get(*i) else {
                    return Vec::new();
                };
                let Some(a) = st.content.ability(id) else {
                    return Vec::new();
                };
                let mut out = vec![head(&a.name)];
                let kind = match a.kind {
                    AbilityKind::Passive => "passive",
                    AbilityKind::Active(_) => "active",
                };
                let from = match source {
                    Source::Innate => "innate".to_string(),
                    Source::Item(item) => format!(
                        "from {}",
                        st.content
                            .item(item)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|| item.to_string())
                    ),
                };
                out.push(dim(format!("{kind} · {from}")));
                if a.cooldown > 0 {
                    out.push(dim(format!("cooldown {}s", a.cooldown.div_ceil(30))));
                }
                if let AbilityKind::Active(active) = a.kind {
                    let effect = match active {
                        Active::Scan { radius } => {
                            format!("scan: paints the map within {radius} tiles")
                        }
                        Active::Ping { radius } => {
                            format!("ping: lights terminals and stairs within {radius} tiles")
                        }
                        Active::Trace { radius } => {
                            format!("trace: arcs to every node within {radius} tiles")
                        }
                    };
                    out.push(Line::from(Span::styled(effect, Style::default().fg(CYAN))));
                }
                out.push(plain(""));
                out.push(plain(a.desc.clone()));
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
        // A popup may have removed the selected row: reselect before drawing, keys or not.
        let rows = self.rows(st);
        if self.row >= rows.len() || rows.get(self.row).is_some_and(|r| r.detail.is_none()) {
            self.row = Self::first_selectable(&rows);
        }
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
                KeyCode::Char(c @ '1'..='7') => {
                    let i = c as usize - '1' as usize;
                    self.set_tab(Tab::ALL[i], st);
                }
                KeyCode::Up | KeyCode::Char('k') => self.move_row(&rows, -1, st.tick),
                KeyCode::Down | KeyCode::Char('j') => self.move_row(&rows, 1, st.tick),
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if let Some(p) = self.interact(st) {
                        return ScreenCmd::Push {
                            to: Box::new(p),
                            fx: TransitionKind::None,
                        };
                    }
                }
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
        let lead = format!(
            "¤ {} · hp {}/{} · ",
            st.character.credits,
            st.character.hp,
            st.character.hp_max(&st.content),
        );
        let load = format!(
            "{:.1}/{:.1} kg",
            st.character.weight(&st.content),
            st.character.carry_cap(&st.content)
        );
        let load_style = if st.character.overburdened(&st.content) {
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        };
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
        let rx = right.saturating_sub((lead.chars().count() + load.chars().count()) as u16 + 1);
        put(buf, rx, inner.y, &lead, Style::default().fg(DIM), right);
        put(
            buf,
            rx + lead.chars().count() as u16,
            inner.y,
            &load,
            load_style,
            right,
        );
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
        let mut starts = [0u16; 7];
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

        // Body, above the status strip and the hints.
        let hint_y = inner.bottom().saturating_sub(1);
        let strip_y = hint_y.saturating_sub(1);
        let body = Rect::new(inner.x, y, inner.width, strip_y.saturating_sub(y));
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
            self.last_list.set((list, first));
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

        // Status strip: the last action's result, typed out; refusals in magenta.
        if let Some(e) = st.log.recent(tick) {
            let p = tick.saturating_sub(e.tick) as f32 / 20.0;
            let text = e.marked();
            let shown = if p < 1.0 {
                format!("{}▮", typewriter(&text, p))
            } else {
                text.clone()
            };
            let style = match e.kind {
                Kind::Warn => Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
                Kind::Story => Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                Kind::Info => Style::default().fg(CYAN),
            };
            put(buf, inner.x + 1, strip_y, &shown, style, right);
        }

        // Hints.
        put(
            buf,
            inner.x + 1,
            hint_y,
            "←→ tabs · ↑↓ select · 1-7 jump · space act · esc close",
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

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    fn last_is_warning(st: &AppState) -> bool {
        st.log.latest().is_some_and(|e| e.kind == Kind::Warn)
    }
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use game::AbilityId;
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

    fn draw(d: &dyn Screen, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    fn walking(st: &mut AppState) {
        let city = st.city.as_ref().unwrap();
        st.player = Some(crate::walker::Player::spawn(city, 1));
    }

    #[test]
    fn deck_tabs_and_rows_render_from_content() {
        let mut st = state();
        let mut d = Deck::new(0);
        st.tick = 40;
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("▌QUESTS▐"), "{text}");
        assert!(
            text.contains("VEX") && text.contains("¤ 320") && text.contains("hp 45/45"),
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
        assert_eq!(d.tab, Tab::Abilities, "left wraps around");
        d.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(d.tab, Tab::Quests, "tab wraps forward");
        d.update(&mut st, &[key(KeyCode::BackTab)]);
        assert_eq!(d.tab, Tab::Abilities);
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
        assert!(
            text.contains("1-7 jump") && text.contains("space act") && text.contains("esc close"),
            "{text}"
        );
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
            '▪', '▫', '≡',
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
            text.contains("Hacking") && text.contains("▰▰▰▰▱ 4"),
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
        // A probe glyph nothing in the deck prints.
        t.draw(|f| {
            let area = f.area();
            let buf = f.buffer_mut();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    buf[(x, y)]
                        .set_char('Ж')
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
            .filter(|(x, y)| buf[(*x, *y)].symbol() == "Ж")
            .count();
        assert!(ghosts > 100, "ghosts {ghosts}: the deck is opaque");
        let (gx, gy) = (110u16, 30u16);
        assert_eq!(buf[(gx, gy)].symbol(), "Ж");
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
        assert!(hacking.text.contains("▰▰▰▰▰ 8"), "{:?}", hacking.text);
        st.tick = 60;
        assert!(draw(&d, &st, 120, 35).contains("▰▰▰▰▰ 8"));
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
        let expect = format!(
            "{:.1} kg · ¤ {}",
            st.character.weight(&st.content),
            st.character.value(&st.content)
        );
        assert_eq!(footer.trim_matches(|c: char| c == '│' || c == ' '), expect);
        d.update(&mut st, &[key(KeyCode::Up)]);
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
        d.update(&mut st, &[key(KeyCode::Up)]);
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

    #[test]
    fn abilities_tab_lists_sources_and_cooldowns() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('7'))]);
        assert_eq!(d.tab, Tab::Abilities);
        st.character.fired.insert(AbilityId::new("netscan"), 0);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("Street sense") && text.contains("passive"),
            "{text}"
        );
        assert!(
            text.contains("Netscan") && text.contains("cooling 8s"),
            "{text}"
        );
        let rows = d.rows(&st);
        d.row = rows
            .iter()
            .position(|r| r.text.contains("Netscan"))
            .unwrap();
        d.changed = 0;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("from Pawnshop deck") && text.contains("within 12 tiles"),
            "{text}"
        );
        st.tick = 400;
        assert!(draw(&d, &st, 120, 35).contains("ready"));
    }

    #[test]
    fn readout_shows_capacity_and_status_strip_shows_messages() {
        let mut st = state();
        let d = Deck::new(0);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("hp 45/45") && text.contains("11.6/16.0 kg"),
            "{text}"
        );
        st.say("equipped Hardhat with visor");
        st.tick += 30;
        assert!(draw(&d, &st, 120, 35).contains("equipped Hardhat with visor"));
        st.warn("over capacity · 20.0/16.0 kg");
        st.tick += 30;
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| d.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let row = |y: u16| {
            (0..120u16)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        };
        let y = (0..35u16)
            .find(|y| row(*y).contains("over capacity"))
            .expect("strip row");
        let x = (0..120u16).find(|x| buf[(*x, y)].symbol() == "o").unwrap();
        assert_eq!(buf[(x, y)].fg, MAGENTA);
        st.character.inventory.push(game::Stack {
            item: ItemId::new("scrap_servo"),
            qty: 40,
        });
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("35.6/16.0 kg"), "{text}");
    }

    #[test]
    fn skills_and_gear_show_perks() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('6'))]);
        let rows = d.rows(&st);
        d.row = rows
            .iter()
            .position(|r| r.text.contains("Hacking"))
            .unwrap();
        d.changed = 0;
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("▰▰▰▰▱ 4") && text.contains("base 3 · +1 Pawnshop deck"),
            "{text}"
        );
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("EYES")).unwrap();
        d.changed = 0;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("+4 sight") && text.contains("grants Low-light"),
            "{text}"
        );
    }

    #[test]
    fn gear_slot_picker_equips_what_fits() {
        let mut st = state();
        walking(&mut st);
        st.character.inventory.push(game::Stack {
            item: ItemId::new("mirrorshades"),
            qty: 1,
        });
        st.character.inventory.push(game::Stack {
            item: ItemId::new("hardhat"),
            qty: 1,
        });
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("EYES")).unwrap();
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the picker")
        };
        st.tick = 40;
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| {
            d.draw(&st, f);
            p.draw(&st, f)
        })
        .unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("a  Mirrorshades")
                && text.contains("u  unequip")
                && text.contains("i  inspect"),
            "{text}"
        );
        assert!(
            !text.contains("Hardhat with visor"),
            "wrong slot must not be offered: {text}"
        );
        p.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert_eq!(
            st.character.equipped(Slot::Eyes),
            Some(&ItemId::new("mirrorshades"))
        );
        assert_eq!(
            last(&st),
            Some("swapped Nightglass optics for Mirrorshades")
        );
        d.row = rows.iter().position(|r| r.text.contains("LEGS")).unwrap();
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(last(&st), Some("nothing in the pack fits legs"));
    }

    #[test]
    fn inventory_verbs_drop_use_and_inspect() {
        let mut st = state();
        walking(&mut st);
        let at = {
            let p = st.player.as_ref().unwrap();
            (p.stratum, p.pos)
        };
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        let rows = d.rows(&st);
        d.row = rows
            .iter()
            .position(|r| r.text.contains("Copper coil"))
            .unwrap();
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the verbs")
        };
        st.tick = 40;
        let text = draw(&*p, &st, 120, 35);
        assert!(
            text.contains("i  inspect")
                && text.contains("d  drop one")
                && text.contains("D  drop all"),
            "{text}"
        );
        assert!(!text.contains("equip") && !text.contains("use"), "{text}");
        p.update(&mut st, &[key(KeyCode::Char('D'))]);
        assert_eq!(st.ground.at(at.0, at.1)[0].qty, 3);
        assert!(!d.rows(&st).iter().any(|r| r.text.contains("Copper coil")));
        let rows = d.rows(&st);
        d.row = rows
            .iter()
            .position(|r| r.text.contains("Stim patch"))
            .unwrap();
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        st.tick += 40;
        let text = draw(&*p, &st, 120, 35);
        assert!(
            text.contains("u  use") && !text.contains("drop all"),
            "{text}"
        );
        st.character.hp = 10;
        p.update(&mut st, &[key(KeyCode::Char('u'))]);
        assert_eq!(st.character.hp, 25);
        d.row = d
            .rows(&st)
            .iter()
            .position(|r| r.text.contains("Dead phone"))
            .unwrap();
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        p.update(&mut st, &[key(KeyCode::Char('i'))]);
        st.tick += 40;
        assert!(draw(&*p, &st, 120, 35).contains("Cracked screen"));
        st.player = None;
        d.row = d
            .rows(&st)
            .iter()
            .position(|r| r.text.contains("Dead phone"))
            .unwrap();
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        p.update(&mut st, &[key(KeyCode::Char('d'))]);
        assert!(last_is_warning(&st));
    }

    #[test]
    fn deck_reselects_after_a_popup_removes_the_row() {
        let mut st = state();
        walking(&mut st);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        let rows = d.rows(&st);
        d.row = rows.len() - 1;
        let last = rows[d.row].text.clone();
        assert!(last.contains("1×"), "the last row is a single item: {last}");
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        p.update(&mut st, &[key(KeyCode::Char('d'))]);
        d.update(&mut st, &[]);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(!text.contains(&last), "{text}");
        assert!(text.contains("▸ "), "selection restored: {text}");
    }

    #[test]
    fn rows_wrap_around() {
        let mut st = state();
        stuff(&mut st);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        let rows = d.rows(&st);
        let first = rows.iter().position(|r| r.detail.is_some()).unwrap();
        let last = rows.iter().rposition(|r| r.detail.is_some()).unwrap();
        assert_eq!(d.row, first);
        d.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(d.row, last, "up from the first row wraps to the last");
        d.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(d.row, first);
    }
}
