//! The deck: the player's menu over the live map — skills, attributes, gear and
//! inventory, workbench, jobs, journal, map, talents. It reopens where it was closed.

use crate::actions::{perform, report};
use crate::anim::effects::{glitch_cells, pulse, typewriter};
use crate::anim::{Easing, Tween};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::messages::Kind;
use crate::render::map::MapRef;
use crate::render::memmap::MemoryMap;
use crate::screens::kit::Kit;
use crate::screens::popup::{Choice, Pick, Popup};
use crate::screens::workbench::Workbench;
use crate::ui::board::{BoardState, RowKind, board_rows};
use crate::ui::chrome::{
    BAR_BG, CYAN, CYAN_RGB, DIM, MAGENTA, MAGENTA_RGB, PANEL_BG, RULE, TEXT, dim, frame, glass,
    head, plain, put, put_paragraph, reveal, warn,
};
use crate::ui::hud::capitalised;
use crate::ui::logo::{self, Logo};
use crate::ui::talents::{TalentsView, TreeCursor, step, talent_at};
use citygen::rng::hash64;
use citygen::{FloorPlan, NodeKind, Place, Pos, StratumKind};
use crossterm::event::{KeyCode, KeyEvent};
use game::{
    AbilityId, AbilityKind, Action, Active, Build, CategoryId, CopyId, Item, ItemCopy, ItemId, Mod,
    Perk, SkillId, Slot, Source, Stat,
};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::cell::Cell;
use std::collections::BTreeSet;

/// Ticks the detail pane, and the deck's glass, take to type a whole readout.
pub(crate) const REVEAL_TICKS: u64 = 20;
/// What the failsafe does, on the deck's glass.
pub(crate) const FAILSAFE_TEXT: &str = "The deck pulls you out at a quarter of your hp when black ICE is on you: focus gone, thirty minutes of cooling, the hp stays lost.";
/// What turning it off costs.
pub(crate) const FAILSAFE_WARN: &str =
    "Off, black ICE can take you to zero. There is no coming back from that.";
/// Ticks a talent's box stays lit after a rank goes in.
const FLASH_TICKS: u64 = 6;
const JITTER_PERIOD: u64 = 90;
const LOGO_MIN_ROWS: u16 = 36;
/// Hotkey letters for picker rows; h j k l move, q closes, u and i are verbs.
const LETTERS: &str = "abcdefgmnoprstvwxyz";
/// The marks after a name, plain then on the cursor's row: a deck's program slots in
/// darker squares, the dot before the sockets, the sockets filled and empty.
const SLOT_USED: (Color, Color) = (Color::Rgb(0x3f, 0x7c, 0x87), Color::Rgb(0x1a, 0x9a, 0xaa));
const SLOT_FREE: (Color, Color) = (Color::Rgb(0x4f, 0x58, 0x64), Color::Rgb(0x2a, 0x74, 0x80));
const MARK_DOT: (Color, Color) = (DIM, Color::Rgb(0x00, 0xa5, 0xb8));
const SOCKET_FILLED: (Color, Color) = (CYAN, CYAN);
const SOCKET_EMPTY: (Color, Color) = (DIM, Color::Rgb(0x00, 0xa5, 0xb8));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Abilities, active and passive.
    Skills,
    /// The five stats and the points left to spend on them.
    Stats,
    /// What you wear and what you carry, and the item under the cursor.
    Kit,
    Workbench,
    Jobs,
    /// Not written yet.
    Journal,
    Map,
    /// The five talent trees, the last tab: on `8`, and on `0` too.
    Talents,
}

impl Tab {
    pub const ALL: [Tab; 8] = [
        Tab::Skills,
        Tab::Stats,
        Tab::Kit,
        Tab::Workbench,
        Tab::Jobs,
        Tab::Journal,
        Tab::Map,
        Tab::Talents,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Skills => "SKILLS",
            Tab::Stats => "STATS",
            Tab::Kit => "GEAR & INVENTORY",
            Tab::Workbench => "WORKBENCH",
            Tab::Jobs => "JOBS",
            Tab::Journal => "JOURNAL",
            Tab::Map => "MAP",
            Tab::Talents => "TALENTS",
        }
    }

    pub fn index(self) -> usize {
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
    Job(crate::jobs::JobId),
    Slot(Slot),
    Item(ItemId),
    /// One copy in the pack, for an item held per copy.
    Copy(CopyId),
    Stat(Stat),
    Category(CategoryId),
    Skill(SkillId),
    /// Index into the journal's flat list of entries.
    Journal(usize),
}

/// One list row and what the detail pane shows for it (`None` = a header).
pub(crate) struct Row {
    pub text: String,
    pub tone: Tone,
    pub detail: Option<Detail>,
    /// Styled text for the list, when the row lights parts of itself.
    pub spans: Option<Line<'static>>,
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
    focus: Option<(Place, Pos)>,
    /// The list pane and its first visible row at the last draw, to anchor popups.
    last_list: Cell<(Rect, usize)>,
    /// GEAR & INVENTORY, with its own cursor, keys and drawing.
    pub(crate) kit: Kit,
    /// Unfolded categories on the SKILLS board; starts as the owned ones.
    open: Option<BTreeSet<CategoryId>>,
    /// The owned categories at the last update, so a fresh unlock unfolds itself.
    owned_seen: Option<BTreeSet<CategoryId>>,
    /// Where the TALENTS tab's cursor sits.
    pub(crate) cursor: TreeCursor,
    /// The talent box lit until its tick: the rank just learned.
    flash: Option<(game::TalentId, u64)>,
    /// The WORKBENCH tab's sheet: the drawing shown and the part selected.
    pub(crate) workbench: Workbench,
}

/// The cursor row's text: cyan, breathing.
pub(crate) fn lit(tick: u64) -> Color {
    CYAN_RGB.scale(0.7 + 0.3 * pulse(tick, 30)).to_color()
}

/// `name` in at most `room` columns, cut with `…` when it is longer.
pub(crate) fn cut(name: &str, room: usize) -> String {
    if name.chars().count() <= room {
        return name.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let kept: String = name.chars().take(room - 1).collect();
    format!("{}…", kept.trim_end())
}

/// What follows an item's name on the rack and in the pack: a deck's program slots, used
/// then free, a dot, then its sockets, filled or empty. Only the worn deck has slots
/// in use; without a copy every socket is empty. Nothing for an item with neither.
pub(crate) fn marks(
    st: &AppState,
    item: &Item,
    copy: Option<&ItemCopy>,
    worn: bool,
    selected: bool,
) -> Vec<Span<'static>> {
    let tone =
        |(plain, lit): (Color, Color)| Style::default().fg(if selected { lit } else { plain });
    let mut out = Vec::new();
    if let Some(deck) = item.deck {
        let used = if worn {
            st.character.loaded_size(&st.content).min(deck.slots)
        } else {
            0
        };
        let free = deck.slots - used;
        if used > 0 {
            out.push(Span::styled("■".repeat(used as usize), tone(SLOT_USED)));
        }
        if free > 0 {
            out.push(Span::styled("□".repeat(free as usize), tone(SLOT_FREE)));
        }
    }
    if !out.is_empty() && !item.sockets.is_empty() {
        out.push(Span::styled(" · ", tone(MARK_DOT)));
    }
    for i in 0..item.sockets.len() {
        let filled = copy.is_some_and(|k| k.fitted.get(i).is_some_and(Option::is_some));
        out.push(if filled {
            Span::styled("■", tone(SOCKET_FILLED))
        } else {
            Span::styled("□", tone(SOCKET_EMPTY))
        });
    }
    out
}

fn tone_style(tone: Tone) -> Style {
    match tone {
        Tone::Normal => Style::default().fg(TEXT),
        Tone::Dim => Style::default().fg(DIM),
        Tone::Warn => Style::default().fg(MAGENTA),
        Tone::Done => Style::default().fg(Color::Rgb(90, 200, 140)),
    }
}

/// Jacked in, the verbs that would touch the gear or the pack are dimmed and the
/// popup's body says why; out of the Net nothing changes and there is no body.
fn hands_off(st: &AppState, choices: &mut [Choice]) -> Vec<String> {
    if !crate::actions::jacked_in(st) {
        return Vec::new();
    }
    for c in choices.iter_mut() {
        if matches!(&c.pick, Pick::Act(a) if crate::actions::handles_items(a)) {
            c.enabled = false;
        }
    }
    vec![capitalised(crate::actions::JACKED_IN)]
}

/// The popup for a gear slot: a picker for what fits it, then Unequip and Inspect for
/// what is worn. Nothing fits and nothing is worn says so in the strip.
pub(crate) fn slot_popup(st: &mut AppState, slot: Slot, anchor: (u16, u16)) -> Option<Popup> {
    let mut choices = Vec::new();
    let mut letters = LETTERS.chars();
    for s in &st.character.inventory {
        let Some(item) = st.content.item(&s.item) else {
            continue;
        };
        if item.slot != Some(slot) {
            continue;
        }
        let Some(k) = letters.next() else { break };
        // A copy is worn as itself, with what is fitted in it.
        let equip = match s.copy_id() {
            Some(copy) => Action::EquipCopy(copy),
            None => Action::Equip(s.item.clone()),
        };
        // The marks tell two copies of one item apart.
        let marks: String = marks(st, item, s.copy.as_ref(), false, false)
            .iter()
            .map(|m| m.content.as_ref())
            .collect();
        let label = if marks.is_empty() {
            item.name.clone()
        } else {
            format!("{} {marks}", item.name)
        };
        choices.push(Choice::new(k, label, Pick::Act(equip)));
    }
    if let Some(id) = st.character.equipped(slot) {
        choices.push(Choice::new(
            'u',
            "Unequip",
            Pick::Act(Action::Unequip(slot)),
        ));
        let worn = Pick::Blueprint {
            item: id.clone(),
            equipped: true,
            copy: st.character.worn_copy(slot).map(|k| k.id),
        };
        choices.push(Choice::new('i', "Inspect", worn));
    }
    if choices.is_empty() {
        st.warn(format!("nothing in the pack fits {}", slot.label()));
        return None;
    }
    let body = hands_off(st, &mut choices);
    Some(
        Popup::verbs(&capitalised(slot.label()), choices, st.tick)
            .with_body(body)
            .anchored(anchor.0, anchor.1),
    )
}

/// The popup for an item in the pack: Inspect, then the verbs it supports.
pub(crate) fn item_popup(st: &mut AppState, id: &ItemId, anchor: (u16, u16)) -> Option<Popup> {
    let item = st.content.item(id)?;
    let qty = st.character.count(id);
    // In the pack, so never the jacked deck: equipping takes it out.
    let carried = Pick::Blueprint {
        item: id.clone(),
        equipped: false,
        copy: None,
    };
    let mut choices = vec![Choice::new('i', "Inspect", carried)];
    if item.slot.is_some() {
        choices.push(Choice::new(
            'e',
            "Equip",
            Pick::Act(Action::Equip(id.clone())),
        ));
    }
    if item.on_use.is_some() {
        choices.push(Choice::new('u', "Use", Pick::Act(Action::Use(id.clone()))));
    }
    choices.push(Choice::new(
        'd',
        "Drop one",
        Pick::Act(Action::Drop {
            item: id.clone(),
            qty: 1,
        }),
    ));
    if qty > 1 {
        choices.push(Choice::new(
            'D',
            "Drop all",
            Pick::Act(Action::Drop {
                item: id.clone(),
                qty,
            }),
        ));
    }
    let body = hands_off(st, &mut choices);
    Some(
        Popup::verbs(&item.name, choices, st.tick)
            .with_body(body)
            .anchored(anchor.0, anchor.1),
    )
}

/// The popup for one copy in the pack; every verb names this copy.
pub(crate) fn copy_popup(st: &mut AppState, copy: CopyId, anchor: (u16, u16)) -> Option<Popup> {
    let stack = st.character.copy_in_pack(copy)?;
    let item = st.content.item(&stack.item)?;
    let carried = Pick::Blueprint {
        item: stack.item.clone(),
        equipped: false,
        copy: Some(copy),
    };
    let mut choices = vec![Choice::new('i', "Inspect", carried)];
    if item.slot.is_some() {
        choices.push(Choice::new(
            'e',
            "Equip",
            Pick::Act(Action::EquipCopy(copy)),
        ));
    }
    choices.push(Choice::new('d', "Drop", Pick::Act(Action::DropCopy(copy))));
    let body = hands_off(st, &mut choices);
    Some(
        Popup::verbs(&item.name, choices, st.tick)
            .with_body(body)
            .anchored(anchor.0, anchor.1),
    )
}

/// The detail pane's card for a row.
pub(crate) fn card(st: &AppState, detail: &Detail) -> Vec<Line<'static>> {
    match detail {
        Detail::Job(id) => crate::jobs::ui::detail(st, *id),
        Detail::Slot(slot) => match st.character.equipped(*slot) {
            Some(id) => Deck::item_card_of(st, id, st.character.worn_copy(*slot)),
            None => vec![
                head("— empty —"),
                dim(format!("nothing in the {} slot", slot.label())),
            ],
        },
        Detail::Item(id) => Deck::item_card(st, id),
        Detail::Copy(copy) => match st.character.copy_in_pack(*copy) {
            Some(s) => Deck::item_card_of(st, &s.item, s.copy.as_ref()),
            None => vec![dim("nothing here")],
        },
        Detail::Journal(i) => Deck::journal_card(st, *i),
        Detail::Category(id) => Deck::category_card(st, id),
        Detail::Skill(id) => Deck::skill_card(st, id),
        Detail::Stat(stat) => {
            let base = st.character.stats.get(*stat);
            let with = st.character.stat(&st.content, *stat);
            let mut out = vec![head(stat.name())];
            out.push(dim(if with == base {
                format!("{base}")
            } else {
                format!("{base} · {with} with gear")
            }));
            out.push(plain(""));
            out.push(plain(st.content.stat_blurb(*stat).to_string()));
            out.push(plain(""));
            out.push(dim("feeds"));
            for s in st.content.skills.iter().filter(|s| s.stat == *stat) {
                let mark = if st.character.has_skill(&s.id) {
                    "◆"
                } else {
                    "◇"
                };
                out.push(plain(format!("{mark} {}", s.name)));
            }
            out
        }
    }
}

/// Draws `rows` into `list`, from row `first`, with the cursor on row `cursor`. Focused,
/// the cursor's row is a lit bar; unfocused, only a dim `▸ ` marks it.
pub(crate) fn draw_rows(
    buf: &mut Buffer,
    list: Rect,
    rows: &[Row],
    first: usize,
    cursor: usize,
    focused: bool,
    tick: u64,
) {
    for (i, row) in rows
        .iter()
        .enumerate()
        .skip(first)
        .take(list.height as usize)
    {
        let ry = list.y + (i - first) as u16;
        let here = i == cursor && row.detail.is_some();
        let selected = here && focused;
        let mut style = tone_style(row.tone);
        if selected {
            style = style.bg(BAR_BG).fg(lit(tick)).add_modifier(Modifier::BOLD);
            for cx in list.x..list.right() {
                buf[(cx, ry)].set_bg(BAR_BG);
            }
        }
        let (mark, mark_style) = match (here, focused) {
            (false, _) => ("  ", style),
            (true, true) => ("▸ ", style),
            (true, false) => ("▸ ", Style::default().fg(DIM)),
        };
        put(buf, list.x + 1, ry, mark, mark_style, list.right());
        if let Some(line) = &row.spans {
            let mut x = list.x + 3;
            for span in &line.spans {
                let mut s = span.style;
                if selected {
                    s = s.bg(BAR_BG).add_modifier(Modifier::BOLD);
                }
                put(buf, x, ry, &span.content, s, list.right());
                x += unicode_width::UnicodeWidthStr::width(span.content.as_ref()) as u16;
            }
        } else {
            put(buf, list.x + 3, ry, &row.text, style, list.right());
        }
    }
}

/// An item's row: `lead` (the slot, or the glyph and count), its name, and the
/// marks after it. The name is cut so the marks always fit the pane.
pub(crate) fn item_row(
    st: &AppState,
    room: usize,
    lead: String,
    name: &str,
    marks: Vec<Span<'static>>,
    selected: bool,
    detail: Detail,
) -> Row {
    if marks.is_empty() {
        return Row {
            text: format!("{lead}{name}"),
            tone: Tone::Normal,
            detail: Some(detail),
            spans: None,
        };
    }
    let marks_w: usize = marks.iter().map(|m| m.content.chars().count()).sum();
    let room = room.saturating_sub(lead.chars().count() + 1 + marks_w);
    let head = format!("{lead}{} ", cut(name, room));
    let text = marks.iter().fold(head.clone(), |t, m| t + &m.content);
    let style = if selected {
        Style::default().fg(lit(st.tick))
    } else {
        tone_style(Tone::Normal)
    };
    let mut spans = vec![Span::styled(head, style)];
    spans.extend(marks);
    Row {
        text,
        tone: Tone::Normal,
        detail: Some(detail),
        spans: Some(Line::from(spans)),
    }
}

impl Deck {
    pub fn new(tick: u64) -> Self {
        Deck {
            tab: Tab::Skills,
            row: 0,
            changed: tick,
            tab_from: 0,
            tab_tween: Tween::new(tick, 1, Easing::OutCubic),
            focus: None,
            last_list: Cell::new((Rect::new(0, 0, 0, 0), 0)),
            kit: Kit::new(tick),
            open: None,
            owned_seen: None,
            cursor: TreeCursor::default(),
            flash: None,
            workbench: Workbench::default(),
        }
    }

    /// The popup for the selected row: a picker for a gear slot, verbs for an item.
    /// Rows with nothing to do give `None`; an empty slot nothing fits says so in the strip.
    /// An item's Inspect opens its blueprint; a skill's or a category's shows its card.
    pub(crate) fn interact(&self, st: &mut AppState) -> Option<Popup> {
        if self.tab == Tab::Kit {
            return self.kit.interact(st);
        }
        let rows = self.rows(st);
        let row = rows.get(self.row)?;
        let (list, first) = self.last_list.get();
        let anchor = (
            list.right() + 2,
            list.y + self.row.saturating_sub(first) as u16,
        );
        match &row.detail {
            Some(Detail::Slot(slot)) => slot_popup(st, *slot, anchor),
            Some(Detail::Item(id)) => item_popup(st, id, anchor),
            Some(Detail::Copy(copy)) => copy_popup(st, *copy, anchor),
            Some(Detail::Category(id)) => {
                let cat = st.content.category(id)?;
                let b = Build::from_character(&st.content, &st.character);
                let mut choices = vec![Choice::new(
                    'i',
                    "Inspect",
                    Pick::Card(Self::category_card(st, id)),
                )];
                if !st.character.has_category(id) {
                    let mut unlock = Choice::new(
                        'u',
                        format!("Unlock · {} SP", cat.cost),
                        Pick::Act(Action::Unlock(id.clone())),
                    );
                    unlock.enabled = b.clone().toggle_category(&st.content, id).is_ok();
                    choices.push(unlock);
                }
                Some(Popup::verbs(&cat.name, choices, st.tick).anchored(anchor.0, anchor.1))
            }
            Some(Detail::Skill(id)) => {
                let skill = st.content.skill(id)?;
                let b = Build::from_character(&st.content, &st.character);
                let mut choices = vec![Choice::new(
                    'i',
                    "Inspect",
                    Pick::Card(Self::skill_card(st, id)),
                )];
                if !st.character.has_skill(id) {
                    let cost = st.content.skill_cost(id).unwrap_or(0);
                    let mut unlock = Choice::new(
                        'u',
                        if cost == 0 {
                            "Unlock · free".to_string()
                        } else {
                            format!("Unlock · {cost} SP")
                        },
                        Pick::Act(Action::UnlockSkill(id.clone())),
                    );
                    unlock.enabled = b.clone().toggle_skill(&st.content, id).is_ok();
                    choices.push(unlock);
                }
                Some(Popup::verbs(&skill.name, choices, st.tick).anchored(anchor.0, anchor.1))
            }
            _ => None,
        }
    }

    /// A category's card: its line, cost, skills with their marks, and what it grants.
    pub(crate) fn category_card(st: &AppState, id: &CategoryId) -> Vec<Line<'static>> {
        let Some(cat) = st.content.category(id) else {
            return vec![warn(format!("unknown category {id}"))];
        };
        let owned = st.character.has_category(id);
        let mut out = vec![head(&cat.name)];
        out.push(dim(if owned {
            "yours".to_string()
        } else {
            format!("{} SP to unlock", cat.cost)
        }));
        out.push(plain(""));
        out.push(plain(cat.line.clone()));
        out.push(plain(""));
        for sk in &cat.skills {
            let name = st
                .content
                .skill(&sk.id)
                .map(|k| k.name.clone())
                .unwrap_or_else(|| sk.id.to_string());
            let cost = if sk.cost == 0 {
                "free".to_string()
            } else {
                format!("{} SP", sk.cost)
            };
            if st.character.has_skill(&sk.id) {
                out.push(Line::from(Span::styled(
                    format!("◆ {name}"),
                    Style::default().fg(CYAN),
                )));
            } else {
                out.push(dim(format!("◇ {name} · {cost}")));
            }
        }
        for a in &cat.grants {
            if let Some(a) = st.content.ability(a) {
                out.push(plain(""));
                out.push(dim(format!("◈ grants {}", a.name)));
                out.push(plain(a.desc.clone()));
            }
        }
        if !cat.unlocks.is_empty() {
            out.push(plain(""));
            out.push(dim("▣ unlocks the workbench"));
        }
        out
    }

    /// A skill's card: its line, its category and what it costs.
    pub(crate) fn skill_card(st: &AppState, id: &SkillId) -> Vec<Line<'static>> {
        let Some(skill) = st.content.skill(id) else {
            return vec![warn(format!("unknown skill {id}"))];
        };
        let mut out = vec![head(&skill.name)];
        let cat = st
            .content
            .category_of(id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let cost = match st.content.skill_cost(id) {
            Some(0) => "free with the category".to_string(),
            Some(n) => format!("{n} SP"),
            None => String::new(),
        };
        out.push(dim(format!("{cat} · {} · {cost}", skill.stat.label())));
        out.push(plain(""));
        out.push(plain(skill.desc.clone()));
        out
    }

    /// Links the deck to a tile: the cursor under the free camera, the player when walking.
    pub fn focus(mut self, place: Place, pos: Pos) -> Self {
        self.focus = Some((place, pos));
        self
    }

    /// Reopens where the deck was closed: the remembered tab and its row, clamped to a
    /// row that still exists and can be selected, and GEAR & INVENTORY where it was left.
    pub fn restore(mut self, st: &AppState) -> Self {
        self.kit = Kit::restore(st);
        self.tab = st.deck_tab;
        self.tab_from = self.tab.index();
        let rows = self.rows(st);
        let want = st.deck_rows[self.tab.index()];
        self.row = if rows.get(want).is_some_and(|r| r.detail.is_some()) {
            want
        } else {
            Self::first_selectable(&rows)
        };
        self.cursor = step(
            &st.content,
            TreeCursor::unpack(st.deck_rows[Tab::Talents.index()]),
            0,
            0,
        );
        self.workbench =
            Workbench::unpack(st.deck_rows[Tab::Workbench.index()]).clamped(&st.content);
        self
    }

    /// What you know and hold, by building: a dim header, then one lit line per entry.
    fn journal_rows(st: &AppState) -> Vec<Row> {
        let groups = crate::credentials::entries(st);
        let mut rows: Vec<Row> = crate::credentials::extra_lines(st)
            .into_iter()
            .map(|text| Row {
                text,
                tone: Tone::Dim,
                detail: None,
                spans: None,
            })
            .collect();
        if groups.is_empty() && rows.is_empty() {
            return vec![Row {
                text: "nothing known yet".into(),
                tone: Tone::Dim,
                detail: None,
                spans: None,
            }];
        }
        let lit = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
        let plain = Style::default().fg(TEXT);
        let mut i = 0;
        for (_, header, es) in groups {
            rows.push(Row {
                text: header,
                tone: Tone::Dim,
                detail: None,
                spans: None,
            });
            for e in es {
                let (text, marks) = crate::messages::parse_marks(&e.text);
                let chars: Vec<char> = text.chars().collect();
                let mut spans = Vec::new();
                let mut at = 0;
                for (a, b) in marks
                    .iter()
                    .copied()
                    .chain(std::iter::once((chars.len(), chars.len())))
                {
                    if at < a {
                        spans.push(Span::styled(chars[at..a].iter().collect::<String>(), plain));
                    }
                    if a < b {
                        spans.push(Span::styled(chars[a..b].iter().collect::<String>(), lit));
                    }
                    at = b;
                }
                rows.push(Row {
                    text,
                    tone: Tone::Normal,
                    detail: Some(Detail::Journal(i)),
                    spans: Some(Line::from(spans)),
                });
                i += 1;
            }
        }
        rows
    }

    /// A journal entry's card: the building, the line, and what it points at.
    fn journal_card(st: &AppState, i: usize) -> Vec<Line<'static>> {
        use crate::credentials::EntryKind;
        let all: Vec<crate::credentials::Entry> = crate::credentials::entries(st)
            .into_iter()
            .flat_map(|(_, _, es)| es)
            .collect();
        let Some(e) = all.get(i) else {
            return vec![warn("nothing here")];
        };
        let name = crate::net::building_name(st, e.building);
        let district = st
            .city
            .as_ref()
            .and_then(|c| {
                c.building_opt(e.building)
                    .map(|b| c.district(b.district).name.clone())
            })
            .unwrap_or_default();
        let mut out = vec![head(&name), dim(district)];
        let (text, _) = crate::messages::parse_marks(&e.text);
        out.push(plain(text));
        match &e.kind {
            EntryKind::Code(c) => {
                if let Some(owner) = st
                    .interiors
                    .homes_of(c.building)
                    .and_then(|h| h.owner_of(c.level, c.number))
                    && let Some(w) = st.world.as_ref()
                {
                    let p = &w.people()[owner.0 as usize];
                    out.push(plain(format!("{}, {}", p.name, p.profession)));
                }
                out.push(plain(format!(
                    "flat {} on {}",
                    c.number,
                    FloorPlan::label(c.level)
                )));
                out.push(plain(format!("code {:04}", c.code)));
            }
            EntryKind::Card(id) => out.extend(Self::item_card(st, id)),
            EntryKind::Backdoor(i) => {
                if let Some(d) = st.backdoors.list.get(*i) {
                    let now = st.net.now(st);
                    let kind = match d.kind {
                        crate::backdoors::Kind::Key { .. } => "a key",
                        crate::backdoors::Kind::Net { .. } => "a Net backdoor",
                    };
                    out.push(plain(kind));
                    out.push(plain(format!("login as {}", d.user_label())));
                    out.push(plain(format!("health {}", d.health(now).label())));
                    if let crate::backdoors::State::Burned { why, .. } = d.state {
                        out.push(plain(format!("burned · {}", why.label())));
                    }
                }
            }
        }
        out
    }

    fn set_tab(&mut self, tab: Tab, st: &AppState) {
        if tab == self.tab {
            return;
        }
        self.tab_from = self.tab.index();
        self.tab = tab;
        self.row = Self::first_selectable(&self.rows(st));
        // The detail pane types on again, and so does the kit's card.
        self.changed = st.tick;
        if tab == Tab::Kit {
            self.kit.reselect(st);
            self.kit.changed = st.tick;
        }
        self.tab_tween = Tween::new(st.tick, 6, Easing::OutCubic);
    }

    fn shift_tab(&mut self, delta: i32, st: &AppState) {
        let n = Tab::ALL.len() as i32;
        let i = (self.tab.index() as i32 + delta).rem_euclid(n) as usize;
        self.set_tab(Tab::ALL[i], st);
    }

    /// Moves the selection by `delta`, skipping header rows; wraps at both ends.
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

    /// Moves the TALENTS cursor along a tier, across trees or between tiers.
    fn move_cursor(&mut self, st: &AppState, dx: i32, dy: i32) {
        self.cursor = step(&st.content, self.cursor, dx, dy);
        self.changed = st.tick;
    }

    pub(crate) fn rows(&self, st: &AppState) -> Vec<Row> {
        match self.tab {
            Tab::Skills => self.board_list(st),
            Tab::Stats => Self::stat_rows(st),
            Tab::Workbench if !st.character.has_workbench(&st.content) => vec![Row {
                text: "no bench. WRENCH unlocks it.".into(),
                tone: Tone::Dim,
                detail: None,
                spans: None,
            }],
            Tab::Jobs => crate::jobs::ui::deck_rows(st),
            Tab::Journal => Self::journal_rows(st),
            Tab::Workbench | Tab::Kit | Tab::Map | Tab::Talents => Vec::new(),
        }
    }

    /// What an item does while worn: one line per perk, then one line naming every
    /// granted ability.
    pub(crate) fn perk_lines(st: &AppState, item: &game::Item) -> Vec<Line<'static>> {
        let cyan = |t: String| Line::from(Span::styled(t, Style::default().fg(CYAN)));
        let mut out = Vec::new();
        for p in &item.perks {
            out.push(cyan(match p {
                Perk::Stat(s, n) => format!("{n:+} {}", s.label()),
                Perk::Sight(n) => format!("{n:+} sight"),
                Perk::Carry(kg) => format!("{kg:+.1} kg carry"),
                Perk::HpMax(n) => format!("{n:+} max hp"),
                Perk::Focus(n) => format!("{n:+} focus"),
            }));
        }
        let names: Vec<String> = item
            .grants
            .iter()
            .map(|id| {
                st.content
                    .ability(id)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| id.to_string())
            })
            .collect();
        if !names.is_empty() {
            out.push(cyan(format!("grants {}", names.join(" · "))));
        }
        out
    }

    fn stat_rows(st: &AppState) -> Vec<Row> {
        let ch = &st.character;
        let b = Build::from_character(&st.content, ch);
        let mut rows = Vec::new();
        if ch.stat_points > 0 || ch.sp > 0 {
            rows.push(Row {
                text: format!("points {} · sp {}", ch.stat_points, ch.sp),
                tone: Tone::Dim,
                detail: None,
                spans: None,
            });
        }
        rows.extend(Stat::ALL.iter().map(|&s| {
            let v = ch.stats.get(s);
            let lvl = (v as usize).min(10);
            let bar = format!("{}{}", "▰".repeat(lvl), "▱".repeat(10 - lvl));
            let next = if ch.stat_points > 0 && v < st.content.creation.stat_cap {
                let n = b.stat_cost(&st.content, s);
                format!("  next {n} pt{}", if n == 1 { "" } else { "s" })
            } else {
                String::new()
            };
            Row {
                text: format!("{:<8} {v:>2}  {bar}{next}", s.name()),
                tone: Tone::Normal,
                detail: Some(Detail::Stat(s)),
                spans: None,
            }
        }));
        rows
    }

    /// The unfolded categories: the remembered set, or the owned ones to start with.
    fn open_set(&self, st: &AppState) -> BTreeSet<CategoryId> {
        self.open
            .clone()
            .unwrap_or_else(|| st.character.categories.clone())
    }

    fn board(&self, st: &AppState) -> Vec<crate::ui::board::BoardRow> {
        let open = self.open_set(st);
        board_rows(
            &st.content,
            &BoardState {
                owned_cats: &st.character.categories,
                owned_skills: &st.character.skills,
                sp_left: st.character.sp as i64,
                open: &open,
            },
            st.tick,
            None,
        )
    }

    /// The SKILLS tab: the board as rows, with the styled lines drawn in their place.
    fn board_list(&self, st: &AppState) -> Vec<Row> {
        self.board(st)
            .into_iter()
            .map(|r| Row {
                text: r.line.to_string(),
                tone: Tone::Normal,
                detail: match (r.selectable, r.kind) {
                    (true, RowKind::Category(id)) => Some(Detail::Category(id)),
                    (true, RowKind::Skill(id)) => Some(Detail::Skill(id)),
                    _ => None,
                },
                spans: Some(r.line),
            })
            .collect()
    }

    /// A category bought through a popup unfolds itself when the deck resumes.
    fn note_new_categories(&mut self, st: &AppState) {
        let now = &st.character.categories;
        if let Some(seen) = &self.owned_seen {
            let fresh: Vec<CategoryId> = now.difference(seen).cloned().collect();
            if !fresh.is_empty() {
                let mut open = self.open_set(st);
                open.extend(fresh);
                self.open = Some(open);
            }
        }
        self.owned_seen = Some(now.clone());
    }

    /// Folds or unfolds the selected category, or the one holding the selected skill.
    fn toggle_fold(&mut self, st: &AppState) {
        let rows = self.rows(st);
        let id = match rows.get(self.row).and_then(|r| r.detail.clone()) {
            Some(Detail::Category(id)) => id,
            Some(Detail::Skill(sid)) => match st.content.category_of(&sid) {
                Some(cat) => cat.id.clone(),
                None => return,
            },
            _ => return,
        };
        let mut open = self.open_set(st);
        if !open.remove(&id) {
            open.insert(id);
        }
        self.open = Some(open);
        self.changed = st.tick;
    }

    /// Everything about one item: the detail pane and the inspect card.
    pub(crate) fn item_card(st: &AppState, id: &ItemId) -> Vec<Line<'static>> {
        Self::item_card_of(st, id, None)
    }

    /// The card of one copy: its sockets read what is fitted in it, and it is
    /// equipped only when that copy is the one worn. Without a copy, every socket the
    /// item is made with reads empty.
    pub(crate) fn item_card_of(
        st: &AppState,
        id: &ItemId,
        copy: Option<&ItemCopy>,
    ) -> Vec<Line<'static>> {
        let Some(item) = st.content.item(id) else {
            return vec![warn(format!("unknown item {id}"))];
        };
        let cyan = |t: String| Line::from(Span::styled(t, Style::default().fg(CYAN)));
        let mut out = vec![head(&item.name)];
        // Where it goes: a slot, or for hardware the socket it fits.
        let place = match (&item.hardware, item.slot) {
            (Some(h), _) => format!(" · {}", h.fits.fits()),
            (None, Some(s)) => format!(" · {}", s.label()),
            (None, None) => String::new(),
        };
        // A copy weighs and costs what is fitted in it too.
        let fitted: Vec<&game::Stack> = copy
            .map(|k| k.fitted.iter().flatten().collect())
            .unwrap_or_default();
        let weight = item.weight + fitted.iter().map(|s| s.weight(&st.content)).sum::<f32>();
        let value = item.value + fitted.iter().map(|s| s.value(&st.content)).sum::<u32>();
        out.push(dim(format!(
            "{}{place} · {weight:.1} kg · ¤ {value}",
            item.kind.label()
        )));
        out.extend(Self::perk_lines(st, item));
        if let Some(h) = &item.hardware {
            out.extend(h.mods.iter().map(|m| cyan(m.line())));
        }
        out.extend(Self::socket_lines(st, item, copy));
        let worn = match copy {
            Some(k) => st.character.is_worn_copy(k.id),
            None => st.character.is_equipped(id),
        };
        if worn {
            out.push(Line::from(Span::styled(
                "equipped",
                Style::default().fg(CYAN),
            )));
        }
        if st.character.loaded.contains(id) {
            out.push(Line::from(Span::styled(
                "loaded",
                Style::default().fg(CYAN),
            )));
        }
        out.push(plain(""));
        out.push(plain(item.desc.clone()));
        out
    }

    /// One line per socket, in order: the piece fitted and what it does, or what the
    /// socket takes.
    fn socket_lines(st: &AppState, item: &Item, copy: Option<&ItemCopy>) -> Vec<Line<'static>> {
        item.sockets
            .iter()
            .enumerate()
            .map(|(i, kind)| {
                let piece = copy
                    .and_then(|k| k.fitted.get(i)?.as_ref())
                    .and_then(|s| st.content.item(&s.item));
                match piece {
                    Some(p) => {
                        let mods = p.hardware.iter().flat_map(|h| h.mods.iter().map(Mod::line));
                        let parts: Vec<String> =
                            std::iter::once(p.name.clone()).chain(mods).collect();
                        Line::from(Span::styled(
                            format!("■ {}", parts.join(" · ")),
                            Style::default().fg(CYAN),
                        ))
                    }
                    None => dim(format!("□ empty socket · {}", kind.takes())),
                }
            })
            .collect()
    }

    /// A program's card: what it does and to what, its numbers, the odds the
    /// character brings against a mid-city construct.
    pub(crate) fn program_card(st: &AppState, id: &ItemId) -> Vec<Line<'static>> {
        let Some(item) = st.content.item(id) else {
            return vec![warn(format!("unknown item {id}"))];
        };
        let Some(p) = item.program else {
            return vec![warn(format!("{} is not a program", item.name))];
        };
        let (targets, typical) = match p.op {
            game::Op::Crack => ("any node or ICE", game::Target::Node(NodeKind::DoorLock)),
            game::Op::Spoof => (
                "door locks and lift controls",
                game::Target::Node(NodeKind::DoorLock),
            ),
            game::Op::Blind => ("cameras", game::Target::Node(NodeKind::Camera)),
            game::Op::Copy => ("servers and vaults", game::Target::Node(NodeKind::Server)),
            game::Op::Backdoor => ("servers", game::Target::Node(NodeKind::Server)),
            game::Op::Attack => ("ICE", game::Target::Guard(citygen::IceGrade::Grey)),
            game::Op::Shield | game::Op::Mask => ("yourself", game::Target::Deck),
        };
        let pw = game::power(&st.character, &st.content, &p, typical);
        let mut out = vec![head(&item.name)];
        out.push(dim(format!("{} · {targets}", p.op.label())));
        out.push(dim(format!(
            "rating {} · size {} · noise {} · work {}",
            p.rating, p.size, p.noise, p.work
        )));
        let df = game::difficulty(110);
        out.push(Line::from(Span::styled(
            format!(
                "your power {pw} vs {df} mid-city · {}% a turn",
                game::chance(pw, df)
            ),
            Style::default().fg(CYAN),
        )));
        if st.character.loaded.contains(id) {
            out.push(Line::from(Span::styled(
                "loaded",
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
            Some(d) => card(st, d),
        }
    }

    /// An ability's card: the sharpened copy, where it comes from, its cooldown and its
    /// effect.
    pub(crate) fn ability_card(
        st: &AppState,
        id: &AbilityId,
        source: &Source,
    ) -> Vec<Line<'static>> {
        // The sharpened copy: talents change the reach and the cooldown.
        let Some(a) = st.character.ability(&st.content, id) else {
            return Vec::new();
        };
        let mut out = vec![head(&a.name)];
        let kind = match a.kind {
            AbilityKind::Passive => "passive",
            AbilityKind::Active(_) => "active",
        };
        let from = match source {
            Source::Innate => "innate".to_string(),
            Source::Category(cat) => format!(
                "from {}",
                st.content
                    .category(cat)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| cat.to_string())
            ),
            Source::Item(item) => format!(
                "from {}",
                st.content
                    .item(item)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| item.to_string())
            ),
            Source::Talent(t) => format!(
                "from {}",
                st.content
                    .talent(t)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| t.to_string())
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

    fn link_line(&self, st: &AppState) -> String {
        let at = self
            .focus
            .or_else(|| st.player.as_ref().map(|p| (p.place, p.pos)));
        let Some((place, pos)) = at else {
            return "▍ LINK · no fix".to_string();
        };
        // Physical strata carry the district on every tile; the Net reads the ground
        // below; a floor is its building's.
        let district = st
            .city
            .as_ref()
            .and_then(|c| {
                let d = match place {
                    Place::Stratum(k) => c
                        .tile(k, pos)
                        .and_then(|t| t.district)
                        .or_else(|| c.tile(StratumKind::Street, pos).and_then(|t| t.district)),
                    Place::Floor { building, .. } | Place::Construct(building) => {
                        c.building_opt(building).map(|b| b.district)
                    }
                };
                d.and_then(|d| c.district_opt(d)).map(|d| d.name.clone())
            })
            .unwrap_or_else(|| "no fix".to_string());
        let label = match place {
            Place::Stratum(k) => k.label().to_string(),
            Place::Floor { level, .. } => FloorPlan::label(level),
            Place::Construct(_) => "construct".to_string(),
        };
        format!("▍ LINK · {district} · {label}")
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
                let map = MapRef::for_place(
                    city,
                    &st.interiors,
                    &st.constructs,
                    p.place,
                    st.minute_of_day(),
                )
                .unwrap_or_else(|| MapRef::stratum_of(city, StratumKind::Street));
                MemoryMap {
                    map: &map,
                    vis: p.vis(),
                    player: Some(p.pos),
                    style: &st.style,
                }
                .render(inner, buf);
                put(
                    buf,
                    body.x + 1,
                    body.y,
                    &format!(" {} · what you remember ", map.label()),
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

impl Deck {
    fn handle(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // A popup may have removed the selected row: reselect before drawing, keys or not.
        let rows = self.rows(st);
        if self.row >= rows.len() || rows.get(self.row).is_some_and(|r| r.detail.is_none()) {
            self.row = Self::first_selectable(&rows);
        }
        if self.tab == Tab::Kit {
            self.kit.reselect(st);
        }
        for k in keys {
            // GEAR & INVENTORY takes the arrows, left and right too, `h j k l` and its
            // filter keys; space, the tab keys, the digits and esc go on to the deck's.
            if self.tab == Tab::Kit {
                self.kit.reselect(st);
                if self.kit.key(st, k.code, st.tick) {
                    continue;
                }
            }
            let rows = self.rows(st);
            if self.row >= rows.len() || rows.get(self.row).is_some_and(|r| r.detail.is_none()) {
                self.row = Self::first_selectable(&rows);
            }
            if self.tab == Tab::Workbench
                && st.character.has_workbench(&st.content)
                && self.workbench.key(st, k.code)
            {
                continue;
            }
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::Glitch,
                    };
                }
                KeyCode::Left | KeyCode::Char('h') if self.tab == Tab::Talents => {
                    self.move_cursor(st, -1, 0)
                }
                KeyCode::Right | KeyCode::Char('l') if self.tab == Tab::Talents => {
                    self.move_cursor(st, 1, 0)
                }
                KeyCode::Up | KeyCode::Char('k') if self.tab == Tab::Talents => {
                    self.move_cursor(st, 0, -1)
                }
                KeyCode::Down | KeyCode::Char('j') if self.tab == Tab::Talents => {
                    self.move_cursor(st, 0, 1)
                }
                KeyCode::Char(' ') | KeyCode::Enter if self.tab == Tab::Talents => {
                    // A rank goes in straight away; the refusal lands in the strip.
                    if let Some(id) = talent_at(&st.content, self.cursor).cloned() {
                        let result = perform(st, Action::Learn(id.clone()));
                        if result.is_ok() {
                            self.flash = Some((id, st.tick + FLASH_TICKS));
                        }
                        report(st, result);
                    }
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.shift_tab(-1, st),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.shift_tab(1, st),
                KeyCode::Char(c @ '0'..='9') => {
                    // By position from `1`; a digit past the last tab does nothing, and
                    // `0` is the last tab too.
                    let i = if c == '0' {
                        Tab::ALL.len() - 1
                    } else {
                        c as usize - '1' as usize
                    };
                    if let Some(tab) = Tab::ALL.get(i) {
                        self.set_tab(*tab, st);
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => self.move_row(&rows, -1, st.tick),
                KeyCode::Down | KeyCode::Char('j') => self.move_row(&rows, 1, st.tick),
                KeyCode::Enter if self.tab == Tab::Skills => self.toggle_fold(st),
                KeyCode::Char(' ') | KeyCode::Enter
                    if matches!(
                        rows.get(self.row).and_then(|r| r.detail.as_ref()),
                        Some(Detail::Stat(_))
                    ) =>
                {
                    // A point buys straight away; the refusal lands in the strip.
                    if let Some(Detail::Stat(s)) = rows.get(self.row).and_then(|r| r.detail.clone())
                    {
                        let result = perform(st, Action::BuyStat(s));
                        report(st, result);
                    }
                }
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
}

/// The status strip: the last action's result while it is fresh, typed out; refusals
/// in magenta. The deck draws it above its hints, the blueprint window inside its frame.
pub(crate) fn status_strip(buf: &mut Buffer, st: &AppState, x: u16, y: u16, right: u16) {
    let Some(e) = st.log.recent(st.tick) else {
        return;
    };
    let p = st.tick.saturating_sub(e.tick) as f32 / 20.0;
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
    put(buf, x, y, &shown, style, right);
}

impl Screen for Deck {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        self.note_new_categories(st);
        let cmd = self.handle(st, keys);
        st.deck_tab = self.tab;
        st.deck_rows[self.tab.index()] = self.row;
        st.deck_kit = self.kit.state;
        st.deck_rows[Tab::Talents.index()] = self.cursor.pack();
        st.deck_rows[Tab::Workbench.index()] = self.workbench.pack();
        cmd
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
                steady: false,
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

        // Tab bar: two cells between labels, one when they would not all fit.
        let labels_w: u16 = Tab::ALL
            .iter()
            .map(|t| t.label().chars().count() as u16 + 2)
            .sum();
        let gap = if labels_w + 2 * (Tab::ALL.len() as u16) < inner.width {
            2
        } else {
            1
        };
        let mut x = inner.x + 1;
        let mut starts = [0u16; Tab::ALL.len()];
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
            x += label.chars().count() as u16 + gap;
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
        } else if self.tab == Tab::Talents {
            TalentsView {
                content: &st.content,
                character: &st.character,
                cursor: self.cursor,
                tick,
                changed: self.changed,
                flash: self.flash.clone(),
            }
            .draw(body, buf);
        } else if self.tab == Tab::Kit {
            self.kit.draw(st, body, buf);
        } else if self.tab == Tab::Workbench && st.character.has_workbench(&st.content) {
            self.workbench.draw(st, body, buf);
        } else {
            let list_w = (body.width * 2 / 5)
                .max(28)
                .min(body.width.saturating_sub(10));
            let list = Rect::new(body.x, body.y, list_w, body.height);
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
            draw_rows(buf, list, &rows, first, self.row, true, tick);
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

        status_strip(buf, st, inner.x + 1, strip_y, right);

        // Hints.
        put(
            buf,
            inner.x + 1,
            hint_y,
            match self.tab {
                Tab::Skills => "←→ tabs · ↑↓ select · enter fold · space act · esc close",
                Tab::Talents => {
                    "←→ along · ↑↓ tiers · tab tabs · 0-8 jump · space learn · esc close"
                }
                Tab::Kit => self.kit.hints(),
                Tab::Workbench if st.character.has_workbench(&st.content) => {
                    self.workbench.hints(st)
                }
                _ => "←→ tabs · ↑↓ select · 0-8 jump · space act · esc close",
            },
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
    use crate::screens::kit::{KitState, Pane};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use game::ItemKind;
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
        assert!(text.contains("▌SKILLS▐"), "{text}");
        assert!(
            text.contains("VEX") && text.contains("¤ 320") && text.contains("hp 45/45"),
            "{text}"
        );
        crate::jobs::testutil::sample_jobs(&mut st);
        d.update(&mut st, &[key(KeyCode::Char('5'))]);
        st.tick += 40;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("The drop") && text.contains("✓ First jack"),
            "{text}"
        );
        assert!(text.contains("LINK"), "{text}");
        assert!(
            draw(&d, &st, 120, 40).contains('█'),
            "tall decks set the handle in block font"
        );

        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        assert_eq!(d.tab, Tab::Kit);
        st.tick += 30;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("▌GEAR & INVENTORY▐")
                && text.contains("PRIMARY")
                && text.contains("Ratchet SMG"),
            "{text}"
        );
        assert!(text.contains("— empty —"), "{text}");
        assert!(text.contains(d.kit.hints()), "the kit's hints: {text}");
        d.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!((d.tab, d.kit.state.tile), (Tab::Kit, 1), "eyes");
        st.tick += 30;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("NIGHTGLASS OPTICS") && text.contains("cyberware · eyes"),
            "{text}"
        );

        // Over the rack's right edge to the pack; right on the pack does nothing.
        d.update(&mut st, &[key(KeyCode::Right); 3]);
        assert_eq!((d.tab, d.kit.state.pane), (Tab::Kit, Pane::Pack));
        st.tick += 30;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("── component ──") && text.contains("3× Copper coil"),
            "{text}"
        );
        assert!(text.contains("←→ gear/pack"), "the pack's hints: {text}");
        // The filter keys are the kit's too.
        d.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert!(d.tab == Tab::Kit && d.kit.state.filter.is_some());
        d.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(d.kit.state.filter, None);
        // The first row is an item, never a header; headers are skipped when moving.
        let rows = d.kit.pack_rows(&st);
        assert!(rows[d.kit.state.row].detail.is_some());
        let before = d.kit.state.row;
        d.update(&mut st, &[key(KeyCode::Down)]);
        assert!(rows[d.kit.state.row].detail.is_some() && d.kit.state.row > before);

        d.update(&mut st, &[key(KeyCode::Left); 4]);
        assert_eq!(
            (d.tab, d.kit.state.pane, d.kit.state.tile),
            (Tab::Kit, Pane::Rack, 0),
            "left never leaves the tab"
        );
        d.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(d.tab, Tab::Workbench, "tab does");
        d.update(&mut st, &[key(KeyCode::Char('0')), key(KeyCode::Tab)]);
        assert_eq!(d.tab, Tab::Skills, "tab wraps forward");
        d.update(&mut st, &[key(KeyCode::BackTab), key(KeyCode::BackTab)]);
        assert_eq!(d.tab, Tab::Map);
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
            text.contains("▌SKILLS▐") && text.contains("SKILLS"),
            "{text}"
        );
        assert!(
            text.contains("enter fold") && text.contains("space act") && text.contains("esc close"),
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
        // The first drawing: everything on hand and the skill is Vex's.
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("WIRE SHIV") && text.contains("needs Electronics · yours"),
            "{text}"
        );
        assert!(text.contains("every part on hand"), "{text}");
        // Two drawings on: the optics are worn, not in the pack.
        d.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Right)]);
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("OPTICS TUNE") && text.contains("1 more Nightglass optics"),
            "{text}"
        );
        assert!(text.contains("not in pack"), "{text}");

        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        d.changed = 0;
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("NET")).unwrap();
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("NET       6  ▰▰▰▰▰▰▱▱▱▱") && text.contains("6 · 7 with gear"),
            "{text}"
        );
        assert!(
            text.contains("◆ Hacking") && text.contains("◇ Vault run"),
            "feeds: {text}"
        );
    }

    #[test]
    fn map_tab_shows_memory_or_no_signal() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('7'))]);
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
        assert!(row.contains("NETRUNNER"));
    }

    #[test]
    fn deck_text_over_a_dim_map_is_not_dimmed() {
        let mut st = state();
        st.tick = 40;
        let d = Deck::new(0);
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        // A remembered map: every cell faint.
        t.draw(|f| {
            let area = f.area();
            let buf = f.buffer_mut();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    buf[(x, y)]
                        .set_char('Ж')
                        .set_fg(Color::Rgb(90, 90, 90))
                        .set_bg(Color::Rgb(40, 40, 40))
                        .modifier
                        .insert(Modifier::DIM);
                }
            }
            d.draw(&st, f);
        })
        .unwrap();
        let buf = t.backend().buffer();
        let faint: Vec<(u16, u16, String)> = buf
            .content
            .iter()
            .enumerate()
            .filter(|(_, c)| c.symbol() != "Ж" && c.modifier.contains(Modifier::DIM))
            .map(|(i, c)| {
                let (x, y) = buf.pos_of(i);
                (x, y, c.symbol().to_string())
            })
            .collect();
        assert!(
            faint.is_empty(),
            "{} deck cells dimmed by the map, first {:?}",
            faint.len(),
            &faint[..faint.len().min(8)]
        );
    }

    /// Forty loose coils: more inventory rows than any pane has.
    fn stuff(st: &mut AppState) {
        use game::Stack;
        st.character
            .inventory
            .extend((0..40).map(|_| Stack::new(ItemId::new("copper_coil"), 1)));
    }

    #[test]
    fn map_tab_ignores_row_keys() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(
            &mut st,
            &[
                key(KeyCode::Char('7')),
                key(KeyCode::Down),
                key(KeyCode::Char('j')),
                key(KeyCode::Up),
            ],
        );
        assert_eq!((d.tab, d.row), (Tab::Map, 0));
        assert!(draw(&d, &st, 100, 30).contains("no signal"));
    }

    #[test]
    fn stats_rows_show_the_stat_and_its_bar() {
        let mut st = state();
        st.character.stats.body = 7;
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let rows = d.rows(&st);
        let body = rows.iter().find(|r| r.text.contains("BODY")).unwrap();
        assert!(body.text.contains(" 7  ▰▰▰▰▰▰▰▱▱▱"), "{:?}", body.text);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("▰▰▰▰▰▰▰▱▱▱") && text.contains("meat"),
            "{text}"
        );
    }

    #[test]
    fn list_scroll_fills_the_pane_at_the_end() {
        let mut st = state();
        stuff(&mut st);
        let mut d = Deck::new(0);
        on_pack(&mut d, &mut st, |r| r.detail.is_some());
        d.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(
            d.kit.state.row,
            d.kit.pack_rows(&st).len() - 1,
            "the last row"
        );
        st.tick = 60;
        let buf = screen(&d, &st, 100, 30);
        let text = buffer_text(&buf);
        let lines: Vec<&str> = text.lines().collect();
        // The pack's rows above the rule: the chips, then the list.
        let rule = lines
            .iter()
            .position(|l| l.contains('╩'))
            .expect("the rule");
        let list: Vec<&str> = lines[..rule]
            .iter()
            .filter_map(|l| l.split('║').nth(1))
            .skip(1)
            .collect();
        assert_eq!(list.len(), 8, "{text}");
        for (i, l) in list.iter().enumerate() {
            assert!(
                !l.trim_matches(|c: char| c == '│' || c == ' ').is_empty(),
                "list row {i} is blank at the end:\n{text}"
            );
        }
        assert!(list[7].contains("▸ "), "the cursor on the last row: {text}");
    }

    #[test]
    fn memory_map_caption_sits_above_the_map() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('7'))]);
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
        let text = draw(
            &Deck::new(0).focus(Place::Stratum(StratumKind::Street), pos),
            &st,
            120,
            35,
        );
        assert!(text.contains(&format!("LINK · {name} · Street")), "{text}");
        // A Net tile carries no district: the ground below names the link.
        let text = draw(
            &Deck::new(0).focus(Place::Stratum(StratumKind::Net), pos),
            &st,
            120,
            35,
        );
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
    fn abilities_show_on_the_board() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert_eq!(d.tab, Tab::Skills);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("◈ Street sense") && text.contains("◈ Signal ghost"),
            "category grants read as notes under their category: {text}"
        );
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
        st.character
            .inventory
            .push(game::Stack::new(ItemId::new("scrap_servo"), 40));
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("35.6/16.0 kg"), "{text}");
    }

    #[test]
    fn skills_and_gear_show_perks() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("NET")).unwrap();
        d.changed = 0;
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("6 · 7 with gear"),
            "the deck's +1 net: {text}"
        );
        on_tile(&mut d, &mut st, Slot::Eyes);
        d.kit.changed = 0;
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
        st.character
            .inventory
            .push(game::Stack::new(ItemId::new("mirrorshades"), 1));
        st.character
            .inventory
            .push(game::Stack::new(ItemId::new("hardhat"), 1));
        let mut d = Deck::new(0);
        on_tile(&mut d, &mut st, Slot::Eyes);
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
                && text.contains("u  Unequip")
                && text.contains("i  Inspect"),
            "{text}"
        );
        // The pack beside the rack lists the hardhat: read the popup alone.
        let text = draw(&*p, &st, 120, 35);
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
        on_tile(&mut d, &mut st, Slot::Legs);
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(last(&st), Some("nothing in the pack fits legs"));
    }

    #[test]
    fn inventory_verbs_drop_and_use() {
        let mut st = state();
        walking(&mut st);
        let at = {
            let p = st.player.as_ref().unwrap();
            (p.place, p.pos)
        };
        let mut d = Deck::new(0);
        on_pack(&mut d, &mut st, |r| r.text.contains("Copper coil"));
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the verbs")
        };
        st.tick = 40;
        let text = draw(&*p, &st, 120, 35);
        assert!(
            text.contains("i  Inspect")
                && text.contains("d  Drop one")
                && text.contains("D  Drop all"),
            "{text}"
        );
        assert!(!text.contains("Equip") && !text.contains("Use"), "{text}");
        p.update(&mut st, &[key(KeyCode::Char('D'))]);
        assert_eq!(st.ground.at(at.0, at.1)[0].qty, 3);
        assert!(
            !d.kit
                .pack_rows(&st)
                .iter()
                .any(|r| r.text.contains("Copper coil"))
        );
        on_pack(&mut d, &mut st, |r| r.text.contains("Stim patch"));
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        st.tick += 40;
        let text = draw(&*p, &st, 120, 35);
        assert!(
            text.contains("u  Use") && !text.contains("Drop all"),
            "{text}"
        );
        st.character.hp = 10;
        p.update(&mut st, &[key(KeyCode::Char('u'))]);
        assert_eq!(st.character.hp, 25);
        st.player = None;
        on_pack(&mut d, &mut st, |r| r.text.contains("Dead phone"));
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
        let last = d
            .kit
            .pack_rows(&st)
            .iter()
            .rev()
            .find(|r| r.text.contains("1×"))
            .expect("a single item somewhere in the pack")
            .text
            .clone();
        on_pack(&mut d, &mut st, |r| r.text == last);
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
    fn tabs_are_in_the_new_order_with_two_new_ones() {
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(
            labels,
            vec![
                "SKILLS",
                "STATS",
                "GEAR & INVENTORY",
                "WORKBENCH",
                "JOBS",
                "JOURNAL",
                "MAP",
                "TALENTS"
            ]
        );
        let mut st = state();
        let mut d = Deck::new(0);
        assert_eq!(d.tab, Tab::Skills);
        // Past the signal-jitter burst of the first ticks.
        st.tick = 40;
        d.update(&mut st, &[key(KeyCode::Char('1'))]);
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("NETRUNNER"), "SKILLS is the board: {text}");
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("BODY"), "STATS lists the five stats: {text}");
        assert!(!text.contains("CYBERDECK"), "no deck tab: {text}");
        d.update(&mut st, &[key(KeyCode::Char('6'))]);
        assert_eq!(d.tab, Tab::Journal);
        let rows = d.rows(&st);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].text.contains("nothing known yet") && rows[0].detail.is_none());
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
        // `8` is the last tab, TALENTS, and `0` still lands there too; `9` is no tab.
        d.update(&mut st, &[key(KeyCode::Char('8'))]);
        assert_eq!(d.tab, Tab::Talents);
        d.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char('0'))]);
        assert_eq!(d.tab, Tab::Talents);
        d.update(&mut st, &[key(KeyCode::Char('2')), key(KeyCode::Char('9'))]);
        assert_eq!(d.tab, Tab::Stats, "9 changes nothing");
        assert_eq!(st.deck_rows.len(), 8);
    }

    #[test]
    fn deck_remembers_its_tab_and_row() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        d.kit.state = KitState {
            pane: Pane::Pack,
            tile: 4,
            row: 0,
            filter: Some(ItemKind::Program),
        };
        d.update(&mut st, &[key(KeyCode::Up)]);
        let kept = d.kit.state;
        assert!(kept.row > 0, "wrapped up to the last program: {kept:?}");
        assert_eq!((st.deck_tab, st.deck_kit), (Tab::Kit, kept));
        let again = Deck::new(5).restore(&st);
        assert_eq!((again.tab, again.kit.state), (Tab::Kit, kept));
        assert_eq!(st.deck_rows.len(), Tab::ALL.len());
        // Close on the last pack row, lose most of the pack, reopen: the row clamps.
        let mut d = Deck::new(6).restore(&st);
        d.kit.state.filter = None;
        d.kit.state.row = 0;
        d.update(&mut st, &[key(KeyCode::Up)]);
        let last = st.deck_kit.row;
        assert!(last > 1, "wrapped up to the last item row: {last}");
        st.character.inventory.truncate(1);
        let clamped = Deck::new(7).restore(&st);
        let rows = clamped.kit.pack_rows(&st);
        let row = clamped.kit.state.row;
        assert!(row < last, "{} rows left", rows.len());
        assert!(rows.get(row).is_some_and(|r| r.detail.is_some()));
    }

    #[test]
    fn stats_tab_spends_points_on_space() {
        let mut st = state();
        st.character.stat_points = 3;
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("points 3") && text.contains("next 3 pts"),
            "{text}"
        );
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("BODY")).unwrap();
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
        assert_eq!((st.character.stats.body, st.character.stat_points), (5, 0));
        assert_eq!(last(&st), Some("BODY 5"));
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
        assert_eq!(st.character.stats.body, 5);
        assert_eq!(
            last(&st),
            Some("no points left"),
            "the refusal lands in the strip"
        );
    }

    #[test]
    fn skills_board_folds_and_unlocks_in_the_deck() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('1'))]);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("◆ NETRUNNER")
                && text.contains("├─ ◆ Hacking")
                && text.contains("◈ Signal ghost"),
            "{text}"
        );
        assert!(
            text.contains("◇ IRON") && !text.contains("Handguns"),
            "unowned categories start folded: {text}"
        );
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("IRON")).unwrap();
        d.update(&mut st, &[key(KeyCode::Enter)]);
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("Handguns") && text.contains("free"), "{text}");
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the verbs")
        };
        p.update(&mut st, &[key(KeyCode::Char('u'))]);
        assert!(
            !st.character.has_category(&game::CategoryId::new("iron")),
            "no SP: nothing unlocks"
        );
        d.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!draw(&d, &st, 120, 35).contains("Handguns"), "folded again");
        st.character.sp = 2;
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        p.update(&mut st, &[key(KeyCode::Char('u'))]);
        assert!(
            st.character.has_category(&game::CategoryId::new("iron"))
                && st.character.has_skill(&game::SkillId::new("handguns"))
        );
        assert_eq!(st.character.sp, 0);
        d.update(&mut st, &[]);
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("◆ IRON") && text.contains("Handguns"),
            "a fresh unlock unfolds itself: {text}"
        );
    }

    #[test]
    fn workbench_is_locked_without_wrench() {
        let mut st = state();
        st.character =
            game::Character::from_template(&st.content, &game::TemplateId::new("kessler")).unwrap();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('4'))]);
        let rows = d.rows(&st);
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].text.contains("no bench. WRENCH unlocks it.") && rows[0].detail.is_none(),
            "{}",
            rows[0].text
        );
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
    }

    #[test]
    fn talents_tab_draws_the_trees_and_moves_the_cursor() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('0'))]);
        assert_eq!(d.tab, Tab::Talents);
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("▌TALENTS▐"), "{text}");
        for name in ["MEAT", "EDGE", "WRENCH", "STREET", "ICE"] {
            assert!(text.contains(name), "{name}: {text}");
        }
        assert!(text.contains("TALENTS · 1 point"), "{text}");
        assert!(
            text.contains("THICK SKIN · MEAT · tier 1 · rank 0/3"),
            "{text}"
        );
        assert!(text.contains("next: +3 max HP"), "{text}");
        assert!(text.contains("3 more"), "a closed tier says so: {text}");
        // The cursor's box is highlighted like a selected row.
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| d.draw(&st, f)).unwrap();
        let lit = t
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|c| c.symbol() == "▲" && c.bg == BAR_BG);
        assert!(lit, "the first box is lit");
        // Right past the tier's end lands in the next tree.
        d.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('l'))]);
        st.tick += 45;
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("QUICK DRAW · EDGE · tier 1"), "{text}");
        // Down: the next tier; the dossier says why it is closed.
        d.update(&mut st, &[key(KeyCode::Char('j'))]);
        st.tick += 45;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("DOUBLE TAP · EDGE · tier 2") && text.contains("3 more points in EDGE"),
            "{text}"
        );
        // Up wraps to the capstone; tab still changes tabs, l did not.
        d.update(&mut st, &[key(KeyCode::Char('k')), key(KeyCode::Char('k'))]);
        st.tick += 45;
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("THE HALF SECOND · EDGE · tier 5"), "{text}");
        d.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(d.tab, Tab::Skills);
    }

    #[test]
    fn talents_tab_learns_on_space_and_shows_the_rank() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('0'))]);
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Stay
        ));
        assert_eq!(
            st.character.talent_rank(&game::TalentId::new("thick_skin")),
            1
        );
        assert_eq!(last(&st), Some("Thick skin 1/3"));
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("▰▱▱ 1/3")
                && text.contains("TALENTS · 0 points")
                && text.contains("MEAT 1"),
            "{text}"
        );
        assert!(text.contains("now: +3 max HP"), "{text}");
        d.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("no points left"));
        assert!(last_is_warning(&st));
        // The smallest screen still draws every tree and the dossier.
        let text = draw(&d, &st, 100, 30);
        assert!(
            text.contains("THICK SKIN · MEAT") && text.contains("ICE") && text.contains("Bruises"),
            "{text}"
        );
        // The cursor survives a reopen.
        d.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('j'))]);
        let d2 = Deck::new(0).restore(&st);
        assert_eq!(d2.tab, Tab::Talents);
        assert_eq!(d2.cursor, d.cursor);
    }

    #[test]
    fn talents_columns_widen_on_a_wide_screen() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('0'))]);
        st.tick = 60;
        let text = draw(&d, &st, 140, 40);
        assert!(text.contains("▲ Thick skin"), "{text}");
        let text = draw(&d, &st, 120, 35);
        assert!(
            text.contains("▲ Thick sk ") && !text.contains("Thick skin"),
            "{text}"
        );
    }

    #[test]
    fn every_tab_label_fits_the_smallest_screen() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('0'))]);
        st.tick = 60;
        let text = draw(&d, &st, 100, 30);
        assert!(text.contains("▌TALENTS▐"), "{text}");
        assert!(
            text.contains("▱▱▱ 0/3"),
            "a box draws at the minimum: {text}"
        );
        d.update(&mut st, &[key(KeyCode::Char('1'))]);
        let text = draw(&d, &st, 100, 30);
        assert!(
            text.contains(" TALENTS ") && text.contains("▌SKILLS▐"),
            "{text}"
        );
    }

    #[test]
    fn inventory_groups_programs_and_marks_the_loaded() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        let rows = d.kit.pack_rows(&st);
        assert!(
            rows.iter().any(|r| r.text == "── program ──"),
            "{:?}",
            rows.iter().map(|r| r.text.clone()).collect::<Vec<_>>()
        );
        on_pack(&mut d, &mut st, |r| r.text.contains("Copy"));
        st.tick = 60;
        let text = draw(&d, &st, 120, 35);
        assert!(text.contains("loaded"), "{text}");
    }

    #[test]
    fn inspect_opens_the_blueprint_window() {
        let mut st = state();
        let mut d = Deck::new(0);
        // The rack: the DECK slot's popup, then Inspect.
        on_tile(&mut d, &mut st, Slot::Deck);
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the slot popup")
        };
        let ScreenCmd::Replace { to: bp, .. } = p.update(&mut st, &[key(KeyCode::Char('i'))])
        else {
            panic!("inspect opens the blueprint window")
        };
        st.tick = 60;
        let text = draw(&*bp, &st, 100, 30);
        assert!(
            text.contains(" BLUEPRINT · PAWNSHOP DECK ") && text.contains("K▒ROK▒WA KX-2"),
            "{text}"
        );

        // The pack: an item's verbs, then Inspect; the card still reads it.
        on_pack(&mut d, &mut st, |r| r.text.contains("Dead phone"));
        st.tick += 40;
        assert!(draw(&d, &st, 120, 35).contains("Cracked screen"));
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the verbs")
        };
        let ScreenCmd::Replace { to: bp, .. } = p.update(&mut st, &[key(KeyCode::Char('i'))])
        else {
            panic!("inspect opens the blueprint window")
        };
        let text = draw(&*bp, &st, 100, 30);
        assert!(
            text.contains(" BLUEPRINT · DEAD PHONE ") && text.contains("MISSING BLUEPRINT"),
            "{text}"
        );

        // A skill is not an item: its Inspect is still the text card.
        d.update(&mut st, &[key(KeyCode::Char('1'))]);
        d.row = d
            .rows(&st)
            .iter()
            .position(|r| r.text.contains("Hacking") && matches!(r.detail, Some(Detail::Skill(_))))
            .unwrap();
        let ScreenCmd::Push { to: mut p, .. } = d.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the verbs")
        };
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('i'))]),
            ScreenCmd::Stay
        ));
        st.tick += 40;
        let text = draw(&*p, &st, 120, 35);
        assert!(
            text.contains("HACKING") && !text.contains("BLUEPRINT"),
            "{text}"
        );
    }

    /// Opens GEAR & INVENTORY with the rack's cursor on `slot`'s tile.
    fn on_tile(d: &mut Deck, st: &mut AppState, slot: Slot) {
        d.update(st, &[key(KeyCode::Char('3'))]);
        d.kit.state.pane = Pane::Rack;
        d.kit.state.tile = Slot::ALL.iter().position(|s| *s == slot).unwrap();
    }

    /// Opens GEAR & INVENTORY with the pack's cursor on the first row `pick` takes.
    fn on_pack(d: &mut Deck, st: &mut AppState, pick: impl Fn(&Row) -> bool) {
        d.update(st, &[key(KeyCode::Char('3'))]);
        d.kit.state.pane = Pane::Pack;
        d.kit.state.row = d.kit.pack_rows(st).iter().position(pick).unwrap();
    }

    /// The detail of the first pack row `pick` takes.
    fn pack_detail(st: &AppState, pick: impl Fn(&Row) -> bool) -> Detail {
        Kit::new(0)
            .pack_rows(st)
            .into_iter()
            .find(|r| pick(r))
            .and_then(|r| r.detail)
            .unwrap()
    }

    /// Inspect on what `at` puts the cursor on, through `on_tile` or `on_pack`: space,
    /// then `i`.
    fn inspect(
        d: &mut Deck,
        st: &mut AppState,
        at: impl FnOnce(&mut Deck, &mut AppState),
    ) -> Box<dyn Screen> {
        at(d, st);
        let ScreenCmd::Push { to: mut p, .. } = d.update(st, &[key(KeyCode::Char(' '))]) else {
            panic!("space opens the popup")
        };
        let ScreenCmd::Replace { to, .. } = p.update(st, &[key(KeyCode::Char('i'))]) else {
            panic!("inspect opens the blueprint window")
        };
        to
    }

    #[test]
    fn jacked_in_gear_and_items_only_inspect() {
        let mut st = state();
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let mut p = crate::walker::Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
        st.character.give(ItemId::new("rebuilt_deck"), 1);
        st.character.give(ItemId::new("stim_patch"), 2);
        let before = st.character.clone();
        let mut d = Deck::new(0);
        let only_inspect = |p: &Popup| {
            assert!(p.choices().len() > 1);
            for c in p.choices() {
                assert_eq!(c.enabled, c.label == "Inspect", "{}", c.label);
            }
        };
        // The rack: the DECK slot offers the spare deck and Unequip, dimmed; Inspect works.
        on_tile(&mut d, &mut st, Slot::Deck);
        let p = d.interact(&mut st).unwrap();
        only_inspect(&p);
        st.tick += 40;
        let text = draw(&p, &st, 120, 35);
        assert!(
            text.contains("Jacked in · gear and pack locked until you jack out"),
            "{text}"
        );
        // The pack: Use and both drops dimmed too.
        on_pack(&mut d, &mut st, |r| r.text.contains("Stim patch"));
        let mut p = d.interact(&mut st).unwrap();
        only_inspect(&p);
        let labels: Vec<&str> = p.choices().iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Inspect", "Use", "Drop one", "Drop all"]);
        // A dimmed verb's key does nothing.
        for k in ['u', 'd', 'D'] {
            assert!(matches!(
                p.update(&mut st, &[key(KeyCode::Char(k))]),
                ScreenCmd::Stay
            ));
        }
        assert_eq!(st.character, before);
    }

    #[test]
    fn inspect_follows_where_the_deck_lies() {
        let mut st = state();
        st.character = game::Character::debug_runner(&st.content);
        // A spare of the model he wears, beside the pawnshop deck in his pack.
        st.character.give(ItemId::new("rebuilt_deck"), 1);
        let loaded = st.character.loaded.clone();
        let mut d = Deck::new(0);
        // From the pack a deck is carried: its blueprint only reads.
        for name in ["Pawnshop deck", "Rebuilt pawn deck"] {
            let mut bp = inspect(&mut d, &mut st, |d, st| {
                on_pack(d, st, |r| r.text.contains(name))
            });
            st.tick += 40;
            let text = draw(&*bp, &st, 100, 30);
            let title = format!(" BLUEPRINT · {} ", name.to_uppercase());
            assert!(
                text.contains(&title) && text.contains("not jacked · equip it first"),
                "{name}: {text}"
            );
            assert!(matches!(
                bp.update(&mut st, &[key(KeyCode::Char(' '))]),
                ScreenCmd::Stay
            ));
            assert_eq!(last(&st), Some("equip the deck to work on it"));
            assert!(last_is_warning(&st));
            assert_eq!(st.character.loaded, loaded);
        }
        // From the rack's DECK tile it is the jacked deck, and works.
        let mut bp = inspect(&mut d, &mut st, |d, st| on_tile(d, st, Slot::Deck));
        st.tick += 40;
        let text = draw(&*bp, &st, 100, 30);
        assert!(
            text.contains(" BLUEPRINT · REBUILT PAWN DECK ")
                && text.contains("[space] eject")
                && !text.contains("not jacked"),
            "{text}"
        );
        bp.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.character.loaded, loaded[1..].to_vec());
    }

    #[test]
    fn the_journal_tab_lists_what_you_know_with_lit_names() {
        let mut st = state();
        let city = st.city.take().unwrap();
        st.enter_city(city);
        walking(&mut st);
        let b = {
            let city = st.city.as_ref().unwrap();
            let world = st.world.as_ref().unwrap();
            city.buildings
                .iter()
                .find(|b| {
                    matches!(
                        b.kind,
                        citygen::BuildingKind::Apartment | citygen::BuildingKind::Megablock
                    ) && world.people().iter().any(|p| p.home == Some(b.id))
                })
                .unwrap()
                .id
        };
        crate::credentials::copy_codes(&mut st, b, 5).unwrap();
        let mut d = Deck::new(0);
        d.set_tab(Tab::Journal, &st);
        let rows = d.rows(&st);
        assert!(rows[0].detail.is_none(), "a header first");
        assert!(
            rows[0]
                .text
                .starts_with(&crate::net::building_name(&st, b).to_uppercase()),
            "{}",
            rows[0].text
        );
        assert!(
            rows.iter()
                .any(|r| r.text.starts_with("door code ") && r.spans.is_some())
        );
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("door code"), "{text}");
        assert!(!text.contains('`'), "marks are stripped: {text}");
        let lines = d.detail(&st, &rows);
        assert!(
            lines.iter().any(|l| l.to_string().contains("flat")),
            "{lines:?}"
        );
    }

    #[test]
    fn an_empty_journal_says_so() {
        let st = state();
        let mut d = Deck::new(0);
        d.set_tab(Tab::Journal, &st);
        assert_eq!(d.rows(&st)[0].text, "nothing known yet");
    }

    /// D'SHAWN: the rebuilt deck worn and loaded full, the pawnshop deck and the
    /// hardware in the pack.
    fn dshawn() -> AppState {
        let mut st = state();
        st.character = game::Character::debug_runner(&st.content);
        st
    }

    /// The first copy of `item` in the pack.
    fn pack_copy(st: &AppState, item: &str) -> game::CopyId {
        st.character
            .inventory
            .iter()
            .find(|s| s.item == ItemId::new(item))
            .and_then(|s| s.copy_id())
            .unwrap_or_else(|| panic!("a copy of {item} in the pack"))
    }

    /// Fits the first `item` in the pack into a socket of `host`.
    fn fit(st: &mut AppState, host: game::Host, socket: usize, item: &str) {
        let piece = pack_copy(st, item);
        perform(
            st,
            Action::Fit {
                host,
                socket,
                piece,
            },
        )
        .unwrap();
    }

    fn screen(d: &Deck, st: &AppState, w: u16, h: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    fn texts(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn inventory_marks_and_hardware() {
        let mut st = dshawn();
        st.character
            .give_item(&st.content, ItemId::new("rebuilt_deck"), 1);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        st.tick = 60;
        let rows = d.kit.pack_rows(&st);
        let texts: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
        assert!(
            texts.iter().any(|t| t.ends_with("⌗  1× Pawnshop deck □□")),
            "no dot without sockets: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.ends_with("⌗  1× Rebuilt pawn deck □□□□□ · □□")),
            "a carried deck's slots all read free: {texts:?}"
        );
        let h = texts
            .iter()
            .position(|t| *t == "── hardware ──")
            .expect("a hardware header");
        assert_eq!(
            texts[h + 1..h + 6],
            [
                "≡  1× Coldfin vI",
                "≡  1× Coldfin vII",
                "≡  1× Sidecar co-processor",
                "≡  1× Hushcan",
                "≡  1× Redeye sight"
            ],
            "after programs"
        );
        // Then his stickers, last: plain stacks, not copies.
        assert_eq!(texts[h + 6], "── sticker ──");
        assert_eq!(texts[h + 7], "◪  2× No Future");
        assert!(texts.iter().position(|t| *t == "── program ──").unwrap() < h);
        assert!(
            rows[h + 1..h + 6]
                .iter()
                .all(|r| matches!(r.detail, Some(Detail::Copy(_))))
        );
        // Drawn as they read, the deck rows and then the hardware at the end.
        on_pack(&mut d, &mut st, |r| r.text.contains("Pawnshop deck"));
        let text = draw(&d, &st, 100, 30);
        assert!(
            text.contains("⌗  1× Pawnshop deck □□ ")
                && text.contains("⌗  1× Rebuilt pawn deck □□□□□ · □□"),
            "{text}"
        );
        // Among the hardware, which his stickers follow now: its header in view.
        d.kit.state.row = h + 3;
        let text = draw(&d, &st, 100, 30);
        assert!(
            text.contains("── hardware ──")
                && text.contains("≡  1× Coldfin vI ")
                && text.contains("≡  1× Coldfin vII")
                && text.contains("≡  1× Sidecar co-processor"),
            "{text}"
        );
    }

    #[test]
    fn two_copies_act_on_their_own_row() {
        let mut st = dshawn();
        walking(&mut st);
        st.character
            .give_item(&st.content, ItemId::new("rebuilt_deck"), 2);
        let decks: Vec<game::CopyId> = st
            .character
            .inventory
            .iter()
            .filter(|s| s.item == ItemId::new("rebuilt_deck"))
            .filter_map(|s| s.copy_id())
            .collect();
        let (first, second) = (decks[0], decks[1]);
        fit(&mut st, game::Host::Pack(second), 0, "sidecar");
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('3'))]);
        let rows = d.kit.pack_rows(&st);
        let row_of = |id| {
            rows.iter()
                .position(|r| r.detail == Some(Detail::Copy(id)))
                .unwrap()
        };
        assert!(row_of(first) < row_of(second));
        assert!(
            rows[row_of(first)].text.ends_with("□□□□□ · □□"),
            "{}",
            rows[row_of(first)].text
        );
        assert!(
            rows[row_of(second)].text.ends_with("□□□□□ · ■□"),
            "{}",
            rows[row_of(second)].text
        );
        on_pack(&mut d, &mut st, |r| r.detail == Some(Detail::Copy(second)));
        let mut p = d.interact(&mut st).unwrap();
        let labels: Vec<&str> = p.choices().iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Inspect", "Equip", "Drop"]);
        let picks: Vec<Pick> = p.choices().iter().map(|c| c.pick.clone()).collect();
        assert!(
            matches!(&picks[0], Pick::Blueprint { item, equipped: false, copy: Some(k) }
                if *k == second && *item == ItemId::new("rebuilt_deck")),
            "{:?}",
            picks[0]
        );
        assert!(
            matches!(&picks[1], Pick::Act(Action::EquipCopy(k)) if *k == second),
            "{:?}",
            picks[1]
        );
        assert!(
            matches!(&picks[2], Pick::Act(Action::DropCopy(k)) if *k == second),
            "{:?}",
            picks[2]
        );
        // Drop takes that copy, the Sidecar with it, and leaves the other.
        p.update(&mut st, &[key(KeyCode::Char('d'))]);
        let (place, pos) = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        assert_eq!(st.ground.at(place, pos)[0].copy_id(), Some(second));
        assert!(st.character.copy_in_pack(second).is_none());
        assert!(st.character.copy_in_pack(first).is_some());
    }

    #[test]
    fn gear_picker_reads_copies_apart() {
        let mut st = dshawn();
        walking(&mut st);
        st.character
            .give_item(&st.content, ItemId::new("rebuilt_deck"), 2);
        let decks: Vec<game::CopyId> = st
            .character
            .inventory
            .iter()
            .filter(|s| s.item == ItemId::new("rebuilt_deck"))
            .filter_map(|s| s.copy_id())
            .collect();
        let (first, second) = (decks[0], decks[1]);
        fit(&mut st, game::Host::Pack(second), 0, "sidecar");
        let mut d = Deck::new(0);
        on_tile(&mut d, &mut st, Slot::Deck);
        let p = d.interact(&mut st).unwrap();
        let rows: Vec<(&str, &Pick)> = p
            .choices()
            .iter()
            .filter(|c| c.label.starts_with("Rebuilt"))
            .map(|c| (c.label.as_str(), &c.pick))
            .collect();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0].0, "Rebuilt pawn deck □□□□□ · □□");
        assert_eq!(rows[1].0, "Rebuilt pawn deck □□□□□ · ■□");
        assert!(matches!(rows[0].1, Pick::Act(Action::EquipCopy(k)) if *k == first));
        assert!(matches!(rows[1].1, Pick::Act(Action::EquipCopy(k)) if *k == second));
    }

    #[test]
    fn cards_list_sockets_and_mods() {
        let mut st = dshawn();
        fit(&mut st, game::Host::Worn(Slot::Deck), 0, "coldfin_2");
        st.character
            .give_item(&st.content, ItemId::new("rebuilt_deck"), 1);
        // The worn deck: its sockets after its grants, then that it is worn.
        let worn = card(&st, &Detail::Slot(Slot::Deck));
        let text = texts(&worn);
        let at = |t: &str| {
            text.iter()
                .position(|l| l == t)
                .unwrap_or_else(|| panic!("{t:?} in {text:?}"))
        };
        let filled = at("■ Coldfin vII · trace per cast −20%");
        let empty = at("□ empty socket · deck hardware");
        assert_eq!(empty, filled + 1);
        let grants = text.iter().rposition(|l| l.starts_with("grants ")).unwrap();
        assert!(grants < filled && empty < at("equipped"), "{text:?}");
        assert_eq!(worn[filled].spans[0].style.fg, Some(CYAN));
        assert_eq!(worn[empty].spans[0].style.fg, Some(DIM));
        // A carried copy of the same deck reads its own empty sockets and is not worn.
        let carried = pack_detail(&st, |r| {
            matches!(r.detail, Some(Detail::Copy(_))) && r.text.contains("Rebuilt")
        });
        let text = texts(&card(&st, &carried));
        assert_eq!(
            text.iter()
                .filter(|l| *l == "□ empty socket · deck hardware")
                .count(),
            2,
            "{text:?}"
        );
        assert!(!text.iter().any(|l| l == "equipped"), "{text:?}");
        // Hardware: what it fits, then what it does.
        let hardware = card(&st, &pack_detail(&st, |r| r.text.ends_with("Coldfin vI")));
        let text = texts(&hardware);
        assert_eq!(text[0], "COLDFIN VI");
        assert_eq!(text[1], "hardware · deck socket · 0.4 kg · ¤ 800");
        let cut = text
            .iter()
            .position(|l| l == "trace per cast −10%")
            .unwrap_or_else(|| panic!("{text:?}"));
        assert_eq!(hardware[cut].spans[0].style.fg, Some(CYAN));
    }

    #[test]
    fn the_card_shares_its_lines() {
        let st = dshawn();
        let card = |slot: Slot| {
            let id = st.character.equipped(slot).cloned().unwrap();
            let copy = st.character.worn_copy(slot);
            texts(&Deck::item_card_of(&st, &id, copy))
        };
        assert_eq!(card(Slot::Primary)[1], "weapon · primary · 3.2 kg · ¤ 900");
        let deck = card(Slot::Deck);
        let grants: Vec<&String> = deck.iter().filter(|l| l.starts_with("grants ")).collect();
        assert_eq!(grants, ["grants Netscan · Arc Trace"], "{deck:?}");
        let cold = pack_copy(&st, "coldfin_1");
        let copy = st.character.copy_in_pack(cold).and_then(|s| s.copy.clone());
        let text = texts(&Deck::item_card_of(
            &st,
            &ItemId::new("coldfin_1"),
            copy.as_ref(),
        ));
        assert_eq!(text[1], "hardware · deck socket · 0.4 kg · ¤ 800");
    }

    #[test]
    fn the_gear_row_and_card_show_the_guns_sockets() {
        let mut st = dshawn();
        let mut d = Deck::new(0);
        on_tile(&mut d, &mut st, Slot::Primary);
        st.tick = 60;
        // The PRIMARY tile's bottom edge, up to its corner.
        let edge = |d: &Deck, st: &AppState| {
            let text = buffer_text(&screen(d, st, 100, 30));
            let lines: Vec<&str> = text.lines().collect();
            let y = lines.iter().position(|l| l.contains("┌ PRIMARY")).unwrap();
            let x = lines[y].chars().position(|c| c == '┌').unwrap();
            let bottom: String = lines[y + 2].chars().skip(x).collect();
            bottom.split('┘').next().unwrap().to_string()
        };
        let row = edge(&d, &st);
        assert!(row.contains(" □□ ") && !row.contains(" · "), "{row:?}");
        let card_of = |st: &AppState| texts(&card(st, &Detail::Slot(Slot::Primary)));
        let text = card_of(&st);
        assert!(text.contains(&"□ empty socket · muzzle hardware".to_string()));
        assert!(text.contains(&"□ empty socket · rail hardware".to_string()));
        fit(&mut st, game::Host::Worn(Slot::Primary), 0, "hushcan");
        let row = edge(&d, &st);
        assert!(row.contains(" ■□ "), "{row:?}");
        let text = card_of(&st);
        assert!(
            text.contains(&"■ Hushcan · cool +1".to_string()),
            "{text:?}"
        );
        assert!(
            text.iter().any(|l| l.ends_with("3.7 kg · ¤ 1600")),
            "{text:?}"
        );
    }

    #[test]
    fn the_hardware_card_names_its_socket_and_stat() {
        let st = dshawn();
        let text = texts(&card(
            &st,
            &pack_detail(&st, |r| r.text.ends_with("Hushcan")),
        ));
        assert_eq!(text[0], "HUSHCAN");
        assert_eq!(text[1], "hardware · muzzle socket · 0.5 kg · ¤ 700");
        assert!(text.contains(&"cool +1".to_string()), "{text:?}");
    }

    #[test]
    fn a_fitted_stat_mod_shows_on_the_stat_card() {
        let mut st = state();
        st.character
            .give_item(&st.content, ItemId::new("hushcan"), 1);
        fit(&mut st, game::Host::Worn(Slot::Primary), 0, "hushcan");
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("COOL")).unwrap();
        let text = texts(&d.detail(&st, &rows));
        assert_eq!(text[1], "5 · 6 with gear", "{text:?}");
    }

    #[test]
    fn draw_rows_marks_an_unfocused_cursor_dim() {
        let row = |text: &str, detail: Option<Detail>| Row {
            text: text.to_string(),
            tone: Tone::Normal,
            detail,
            spans: None,
        };
        let rows = vec![
            row("HEAD", None),
            row("one", Some(Detail::Stat(Stat::ALL[0]))),
            row("two", Some(Detail::Stat(Stat::ALL[1]))),
        ];
        let list = Rect::new(0, 0, 30, 3);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
        draw_rows(&mut buf, list, &rows, 0, 1, true, 0);
        assert_eq!(buf[(1, 1)].symbol(), "▸");
        assert_eq!(buf[(1, 1)].bg, BAR_BG);
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
        draw_rows(&mut buf, list, &rows, 0, 1, false, 0);
        assert_eq!(buf[(1, 1)].symbol(), "▸");
        assert_eq!(buf[(1, 1)].fg, DIM);
        assert_ne!(buf[(1, 1)].bg, BAR_BG);
    }
}
