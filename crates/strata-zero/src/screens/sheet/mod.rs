//! ATTRIBUTES, the character sheet: on the left the attributes you buy, the secondary numbers
//! gear and chrome move and the resistances, as rack tiles with a readout of the one
//! selected; on the right the ledger of the chrome in you (`ledger`).

pub mod ledger;

pub use ledger::LedgerRow;

use crate::actions::{perform, report};
use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use crate::screens::create::{attribute_colour, wrap};
use crate::screens::deck::{REVEAL_TICKS, cut, lit};
use crate::screens::popup::wrapped_rows;
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT, put, put_paragraph, reveal, solid,
};
// A bonus reads green, what a choice would cost amber: the HUD's own.
use crate::clinic::Operating;
use crate::ui::hud::{AMBER, GREEN};
use citygen::{Place, StratumKind};
use crossterm::event::KeyCode;
use game::{Action, BotBody, Build, Contrib, Num, Source};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::ops::Range;

/// Columns of the left pane, from one in from the body's edge; one more stays clear
/// before the rule.
const PANE_W: u16 = 43;
/// The rule between the panes, from the body's left edge.
const RULE_X: u16 = 45;
/// The rule over the readout, right under the last block's tiles (it separates them
/// itself), and the readout under it, from the body's top.
const READOUT_RULE: u16 = 17;
const READOUT_Y: u16 = 18;

/// The left pane's blocks of tiles, in `Num::ALL` order: a header's title, the row
/// it sits on, and each tile's width. A blank row parts one block from the next.
const BLOCKS: [(&str, u16, u16); 3] = [
    ("ATTRIBUTES", 0, 8),
    ("SECONDARY", 6, 7),
    ("RESISTANCES", 12, 7),
];
/// A block's tiles, from its header's row: a blank row between the two.
const TILES_DY: u16 = 2;

/// The block a number's tile sits in.
fn block_of(n: Num) -> usize {
    match n {
        Num::Attribute(_) => 0,
        Num::Resist(_) => 2,
        _ => 1,
    }
}

/// The tiles of a block, as indices into `Num::ALL`, which keeps each block together.
fn span(b: usize) -> Range<usize> {
    let first = Num::ALL.iter().position(|&n| block_of(n) == b).unwrap_or(0);
    let len = Num::ALL.iter().filter(|&&n| block_of(n) == b).count();
    first..first + len
}

/// `n` in words up to twelve, in digits past it.
fn count(n: usize) -> String {
    const WORDS: [&str; 12] = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve",
    ];
    match n {
        1..=12 => WORDS[n - 1].to_string(),
        _ => n.to_string(),
    }
}

/// Green up, magenta down, plain at nothing.
fn signed(n: i32) -> Color {
    match n.signum() {
        1 => GREEN,
        -1 => MAGENTA,
        _ => TEXT,
    }
}

/// One number as the sheet reads it: with everything, what it is of its own, where the
/// rest comes from, and why the two don't add up when they don't.
struct Reading {
    num: Num,
    value: i32,
    /// What it is before gear, chrome and talents: 0 for armour, `None` for a
    /// resistance (nothing but theirs) and for sight with no body to see from.
    own: Option<i32>,
    sources: Vec<(Contrib, i32)>,
    /// Abilities that move it right now: the kind of what grants it, the ability's
    /// name, what grants it, and how much. Low-light's sight, while the lamps are on.
    boons: Vec<(usize, String, Option<String>, i32)>,
    /// Why the value is not its own plus its sources: a cap, a floor, a construct.
    note: Option<Note>,
}

/// Why a number is not its own plus its sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Note {
    Capped,
    Floor,
    Construct,
    NoBody,
}

impl Note {
    /// The note on its own line: `capped at 10`.
    fn line(self, value: i32) -> String {
        match self {
            Note::Capped => format!("capped at {value}"),
            Note::Floor => format!("at least {value}"),
            Note::Construct => format!("in a construct: {value}"),
            Note::NoBody => format!("no body: {value}"),
        }
    }

    /// The note folded into line one, after the value: `REFLEX 9 · 10 in all, capped`.
    fn folded(self) -> &'static str {
        match self {
            Note::Capped => "capped",
            Note::Floor => "at its floor",
            Note::Construct => "in a construct",
            Note::NoBody => "no body",
        }
    }
}

impl Reading {
    fn of(st: &AppState, num: Num) -> Reading {
        let (c, ch) = (&st.content, &st.character);
        let sources = ch.sources(c, num);
        let place = st.player.as_ref().map(|p| p.place);
        let value = match num {
            Num::Attribute(s) => ch.attribute(c, s) as i32,
            Num::Hp => ch.hp_max(c) as i32,
            Num::Focus => ch.focus_max(c) as i32,
            Num::Armor => ch.armor(c),
            Num::Sight => sight(st),
            Num::Carry => ch.carry_cap(c).round() as i32,
            Num::Move => {
                let at = place.unwrap_or(Place::Stratum(StratumKind::Street));
                crate::net::you_move(st, at) as i32
            }
            Num::Resist(h) => ch.resist(c, h),
        };
        let boons: Vec<_> = match num {
            Num::Sight => st
                .lowlight()
                .map(|(id, n, from)| {
                    let name = c
                        .ability(&id)
                        .map_or_else(|| id.to_string(), |a| a.name.clone());
                    let (kind, by) = granted(st, &from);
                    (kind, name, by, n)
                })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        let added = sources.iter().map(|(_, v)| v).sum::<i32>()
            + boons.iter().map(|(.., v)| v).sum::<i32>();
        let own = match num {
            Num::Attribute(s) => Some(ch.attributes.get(s) as i32),
            Num::Hp => Some(ch.hp_base as i32),
            Num::Focus => Some(ch.focus_base as i32),
            Num::Armor => Some(0),
            Num::Sight => sight_here(st),
            Num::Carry => Some(value - added),
            Num::Move => Some(crate::net::BASE_MOVE as i32),
            Num::Resist(_) => None,
        };
        let note = match (num, own.map(|o| o + added)) {
            (_, Some(sum)) if sum == value => None,
            (Num::Move, Some(_)) if place.is_some_and(Place::in_construct) => Some(Note::Construct),
            (_, Some(sum)) if sum > value => Some(Note::Capped),
            (_, Some(_)) => Some(Note::Floor),
            (Num::Sight, None) if added != 0 => Some(Note::NoBody),
            _ => None,
        };
        Reading {
            num,
            value,
            own,
            sources,
            boons,
            note,
        }
    }

    /// What the value moved from its own: the tile's bottom edge. A resistance is
    /// nothing but its sources, so it has none.
    fn edge(&self) -> Option<i32> {
        self.own.map(|o| self.value - o).filter(|e| *e != 0)
    }

    /// The value's colour: an attribute plain, a secondary number by its edge, a
    /// resistance by its sign.
    fn colour(&self) -> Color {
        match self.num {
            Num::Attribute(_) => TEXT,
            Num::Resist(_) => signed(self.value),
            _ => signed(self.edge().unwrap_or(0)),
        }
    }

    /// The tile's bottom edge, as text and colour.
    fn modifier(&self) -> Option<(String, Color)> {
        self.edge().map(|e| (format!("{e:+}"), signed(e)))
    }

    /// The kinds of source that move it, in `KINDS` order.
    fn kinds(&self) -> Vec<usize> {
        let mut kinds: Vec<usize> = self
            .sources
            .iter()
            .map(|(f, _)| kind_of(f))
            .chain(self.boons.iter().map(|(k, ..)| *k))
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    }
}

/// The place's own sight radius at this hour, before any bonus: what the field of
/// view starts from. No body, nothing.
fn sight_here(st: &AppState) -> Option<i32> {
    let p = st.player.as_ref()?;
    let daylight = st
        .city
        .as_ref()
        .and_then(|city| {
            MapRef::for_place(
                city,
                &st.interiors,
                &st.constructs,
                p.place,
                st.minute_of_day(),
            )
        })
        .map_or(0.0, |map| map.daylight());
    Some(p.radius_at(daylight))
}

/// The radius you see by where you stand, as the map's field of view reads it; no
/// body, no sight.
fn sight(st: &AppState) -> i32 {
    sight_here(st).map_or(0, |here| (here + st.sight_bonus()).max(1))
}

/// The kinds of source, in the order the readout lists them, each with its word: a
/// docked bot by its body's.
const KINDS: [&str; 8] = [
    "gear",
    "hardware",
    "chrome",
    BotBody::Air.word(),
    BotBody::Ground.word(),
    "talents",
    "category",
    "innate",
];

/// A bot's kind of source, by its body.
fn bot_kind(body: BotBody) -> usize {
    match body {
        BotBody::Air => 3,
        BotBody::Ground => 4,
    }
}

fn kind_of(from: &Contrib) -> usize {
    match from {
        Contrib::Gear(_) => 0,
        Contrib::Hardware(_) => 1,
        Contrib::Chrome(_) | Contrib::Pieces(_) => 2,
        Contrib::Bot(_, body) => bot_kind(*body),
        Contrib::Talents => 5,
    }
}

/// The kind and the name of what grants an ability: an item is gear, chrome or a
/// docked bot.
fn granted(st: &AppState, from: &Source) -> (usize, Option<String>) {
    let c = &st.content;
    match from {
        Source::Item(id) => {
            let item = c.item(id);
            let kind = match item {
                Some(i) if i.implant.is_some() => 2,
                Some(i) => i.bot.as_ref().map_or(0, |b| bot_kind(b.body)),
                None => 0,
            };
            let name = item.map_or_else(|| id.to_string(), |i| i.name.clone());
            (kind, Some(name))
        }
        Source::Talent(t) => (
            5,
            Some(
                c.talent(t)
                    .map_or_else(|| t.to_string(), |t| t.name.clone()),
            ),
        ),
        Source::Category(k) => (
            6,
            Some(
                c.category(k)
                    .map_or_else(|| k.to_string(), |k| k.name.clone()),
            ),
        ),
        Source::Innate => (7, None),
    }
}

/// `lead`, then `items` with `sep` between them, wrapped at `width`: a line that
/// wraps goes on under the first item.
fn flow(
    lead: Span<'static>,
    items: Vec<Span<'static>>,
    sep: Span<'static>,
    width: usize,
) -> Vec<Line<'static>> {
    let indent = lead.width();
    let mut out = Vec::new();
    let mut line = vec![lead];
    let mut used = indent;
    let mut empty = true;
    for item in items {
        let w = item.width();
        if !empty && used + sep.width() + w > width {
            out.push(Line::from(std::mem::take(&mut line)));
            line.push(Span::raw(" ".repeat(indent)));
            used = indent;
            empty = true;
        }
        if !empty {
            used += sep.width();
            line.push(sep.clone());
        }
        let room = width.saturating_sub(used);
        used += w.min(room);
        line.push(Span::styled(cut(&item.content, room), item.style));
        empty = false;
    }
    out.push(Line::from(line));
    out
}

/// `items`, each kept whole, ` · `-joined `sep` between them, wrapped at `width`; an
/// item wider than a line is cut.
fn run(items: Vec<Vec<Span<'static>>>, sep: Span<'static>, width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut line: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for item in items {
        let w: usize = item.iter().map(Span::width).sum();
        if used > 0 && used + sep.width() + w > width {
            out.push(Line::from(std::mem::take(&mut line)));
            used = 0;
        }
        if used > 0 {
            used += sep.width();
            line.push(sep.clone());
        }
        for span in item {
            let text = cut(&span.content, width.saturating_sub(used));
            used += text.chars().count();
            line.push(Span::styled(text, span.style));
        }
    }
    if !line.is_empty() {
        out.push(Line::from(line));
    }
    out
}

/// `line` within `width`, ending in `…`: what came after it was cut.
fn marked_cut(line: &Line<'static>, width: usize) -> Line<'static> {
    let mut cells: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |c| (c, s.style)))
        .collect();
    cells.truncate(width.saturating_sub(1));
    while cells.last().is_some_and(|&(c, _)| c == ' ') {
        cells.pop();
    }
    let style = cells.last().map_or(Style::default().fg(TEXT), |&(_, s)| s);
    cells.push(('…', style));
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (c, s) in cells {
        match spans.last_mut() {
            Some(last) if last.style == s => last.content.to_mut().push(c),
            _ => spans.push(Span::styled(c.to_string(), s)),
        }
    }
    Line::from(spans)
}

/// A number's readout, in parts so it can give way when rows run short: line one, the
/// blurb, what an attribute feeds, and where the rest comes from with any note.
struct Readout {
    /// Line one's left: the name, the value, and what it is with everything.
    head: Vec<Span<'static>>,
    /// Whether line one goes on past the value (`· 10 in all`): a folded note joins it
    /// with a comma, else with a dot.
    with: bool,
    /// What the next point costs, at the right of line one.
    next: Option<String>,
    blurb: Vec<Line<'static>>,
    feeds: Vec<Line<'static>>,
    /// Where the rest comes from: each kind's word with its entries, then each ability
    /// with what grants it.
    sources: Vec<(Span<'static>, Vec<Span<'static>>)>,
    /// Why the value is not its own plus its sources, and the value.
    note: Option<(Note, i32)>,
}

impl Readout {
    /// The readout `width` columns wide.
    fn of(st: &AppState, r: &Reading, width: usize) -> Readout {
        let (c, ch) = (&st.content, &st.character);
        let bold = |colour: Color| Style::default().fg(colour).add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(DIM);
        let name = match r.num {
            Num::Attribute(s) => attribute_colour(s),
            _ => r.colour(),
        };
        // Its own value and the value with everything, while they differ and it has
        // one of its own to speak of; else the value.
        let mut head = vec![Span::styled(format!("{} ", r.num.name()), bold(name))];
        let with = match r.own {
            Some(own) if own != r.value && r.num != Num::Armor => {
                let with = match r.kinds()[..] {
                    [k] => format!("with {}", KINDS[k]),
                    _ => "in all".to_string(),
                };
                head.push(Span::styled(own.to_string(), bold(TEXT)));
                head.push(Span::styled(format!(" · {} {with}", r.value), dim));
                true
            }
            _ => {
                head.push(Span::styled(r.value.to_string(), bold(r.colour())));
                false
            }
        };
        // What the next point costs, at the right, while there are points to spend.
        let next = match r.num {
            Num::Attribute(s)
                if ch.attribute_points > 0 && ch.attributes.get(s) < c.creation.attribute_cap =>
            {
                let n = Build::from_character(c, ch).attribute_cost(c, s);
                Some(format!("next {n} pt{}", if n == 1 { "" } else { "s" }))
            }
            _ => None,
        };

        let blurb = match r.num {
            Num::Attribute(s) => c.attribute_blurb(s),
            n => c.number_blurb(n.label()),
        };
        let blurb = wrap(blurb, width)
            .into_iter()
            .map(|l| Line::from(Span::styled(l, Style::default().fg(TEXT))))
            .collect();

        let mut feeds = Vec::new();
        if let Num::Attribute(s) = r.num {
            let skills = c.skills.iter().filter(|k| k.attribute == s).map(|k| {
                let mark = if ch.has_skill(&k.id) { "◆" } else { "◇" };
                Span::styled(format!("{mark} {}", k.name), Style::default().fg(TEXT))
            });
            feeds = flow(
                Span::styled("feeds ", dim),
                skills.collect(),
                Span::raw(" "),
                width,
            );
        }

        let item = |id: &game::ItemId| {
            c.item(id)
                .map_or_else(|| id.to_string(), |i| i.name.clone())
        };
        let mut sources = Vec::new();
        for (k, word) in KINDS.iter().enumerate() {
            let entries: Vec<Span<'static>> = r
                .sources
                .iter()
                .filter(|(f, _)| kind_of(f) == k)
                .map(|(from, v)| {
                    let text = match from {
                        Contrib::Gear(id)
                        | Contrib::Chrome(id)
                        | Contrib::Hardware(id)
                        | Contrib::Bot(id, _) => {
                            format!("{v:+} {}", item(id))
                        }
                        Contrib::Talents => format!("{v:+}"),
                        Contrib::Pieces(n) => {
                            let s = if *n == 1 { "" } else { "s" };
                            format!("{v} · {} piece{s}", count(*n))
                        }
                    };
                    Span::styled(text, Style::default().fg(signed(*v)))
                })
                .collect();
            if !entries.is_empty() {
                sources.push((Span::styled(format!("{word} "), dim), entries));
            }
        }
        // An ability's part is its own, named with what grants it.
        for (k, ability, by, v) in &r.boons {
            let by = by.as_ref().map_or(String::new(), |b| format!(" · {b}"));
            sources.push((
                Span::styled(format!("{} ", KINDS[*k]), dim),
                vec![Span::styled(
                    format!("{v:+} {ability}{by}"),
                    Style::default().fg(signed(*v)),
                )],
            ));
        }
        Readout {
            head,
            with,
            next,
            blurb,
            feeds,
            sources,
            note: r.note.map(|n| (n, r.value)),
        }
    }

    /// Line one `width` wide, the next point's cost at the right; `fold` takes the
    /// note in after the value, in the note's colour.
    fn line_one(&self, width: usize, fold: bool) -> Line<'static> {
        let mut spans = self.head.clone();
        if let Some((note, _)) = self.note.filter(|_| fold) {
            let join = if self.with { ", " } else { " · " };
            spans.push(Span::styled(
                format!("{join}{}", note.folded()),
                Style::default().fg(FAINTER),
            ));
        }
        if let Some(next) = &self.next {
            let used: usize = spans.iter().map(Span::width).sum();
            let pad = width.saturating_sub(used + next.chars().count()).max(2);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(next.clone(), Style::default().fg(AMBER)));
        }
        Line::from(spans)
    }

    /// Where the rest comes from: a line or more a kind, or `flowed`, every kind in one
    /// run, ` · ` between, each kind's word leading its first entry.
    fn source_lines(&self, width: usize, flowed: bool) -> Vec<Line<'static>> {
        let sep = Span::styled(" · ", Style::default().fg(DIM));
        if !flowed {
            return self
                .sources
                .iter()
                .flat_map(|(lead, entries)| flow(lead.clone(), entries.clone(), sep.clone(), width))
                .collect();
        }
        let items = self.sources.iter().flat_map(|(lead, entries)| {
            entries.iter().enumerate().map(|(i, e)| match i {
                0 => vec![lead.clone(), e.clone()],
                _ => vec![e.clone()],
            })
        });
        run(items.collect(), sep, width)
    }

    /// The note on a line of its own.
    fn note_line(&self, width: usize) -> Option<Line<'static>> {
        self.note.map(|(note, value)| {
            Line::from(Span::styled(
                cut(&note.line(value), width),
                Style::default().fg(FAINTER),
            ))
        })
    }

    /// The lines in at most `height` rows of `width`, counted as they wrap. Short of
    /// rows the blank line gives way first, then the blurb from its end (marked cut),
    /// then what an attribute feeds, so where the number comes from stays to the last.
    /// Still short, the note folds into line one, then the sources flow onto fewer
    /// lines; only then is the last line that fits marked cut.
    fn fit(mut self, width: usize, height: usize) -> Vec<Line<'static>> {
        let rows = |lines: &[Line<'static>]| -> usize {
            lines.iter().map(|l| wrapped_rows(l, width)).sum()
        };
        let mut blank = !self.blurb.is_empty()
            && !(self.feeds.is_empty() && self.sources.is_empty() && self.note.is_none());
        let mut cut_blurb = false;
        let (mut fold, mut flowed) = (false, false);
        loop {
            let mut lines = vec![self.line_one(width, fold)];
            lines.extend(self.blurb.iter().cloned());
            if cut_blurb && let Some(last) = lines.last_mut().filter(|_| !self.blurb.is_empty()) {
                let text = format!("{last}…");
                *last = Line::from(Span::styled(cut(&text, width), Style::default().fg(TEXT)));
            }
            if blank {
                lines.push(Line::from(""));
            }
            lines.extend(self.feeds.iter().cloned());
            lines.extend(self.source_lines(width, flowed));
            if !fold {
                lines.extend(self.note_line(width));
            }
            if rows(&lines) <= height {
                return lines;
            }
            if blank {
                blank = false;
            } else if self.blurb.pop().is_some() {
                cut_blurb = true;
            } else if self.feeds.pop().is_some() {
                // A line of what it feeds gone; try again.
            } else if !fold && self.note.is_some() {
                fold = true;
            } else if !flowed {
                flowed = true;
            } else {
                while rows(&lines) > height && lines.pop().is_some() {}
                if let Some(last) = lines.last_mut() {
                    *last = marked_cut(last, width);
                }
                return lines;
            }
        }
    }

    /// Where the rest comes from as `fit` keeps it whatever the rows: every entry, and
    /// the note's word.
    #[cfg(test)]
    fn says(&self) -> Vec<String> {
        let entries = self
            .sources
            .iter()
            .flat_map(|(_, e)| e.iter().map(|s| s.content.to_string()));
        entries
            .chain(self.note.map(|(n, _)| n.folded().to_string()))
            .collect()
    }
}

/// Which pane holds the cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetPane {
    #[default]
    Left,
    Ledger,
}

/// Where the cursor is, kept between visits to the tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SheetState {
    pub pane: SheetPane,
    /// The left pane's tile, an index into `Num::ALL`.
    pub tile: usize,
    /// The ledger's row: a site or a socket, kept as itself so it stays put when
    /// the rows above it change.
    pub row: LedgerRow,
    /// The picker's pick, while it is open.
    pub picker: Option<usize>,
}

pub struct Sheet {
    pub state: SheetState,
    /// Tick of the last change of tile: drives the readout's typewriter.
    pub changed: u64,
    /// Tick of the last change of the ledger's card: its own typewriter.
    pub card: u64,
    /// The row `x` asked about: a second `x` there pulls it out and pays the doc.
    pub pulling: Option<LedgerRow>,
}

impl Sheet {
    pub fn new(tick: u64) -> Sheet {
        Sheet {
            state: SheetState::default(),
            changed: tick,
            card: tick,
            pulling: None,
        }
    }

    /// Reopens where the tab was left (`st.deck_sheet`): on the left pane, with the
    /// tile it had. The readout types out afresh.
    pub fn restore(st: &AppState) -> Sheet {
        let mut sheet = Sheet::new(st.tick);
        sheet.state = st.deck_sheet;
        sheet.opened(st.tick);
        sheet
    }

    /// The tab comes up: on the left pane with the last tile, no picker open, the
    /// readout typing.
    pub(crate) fn opened(&mut self, tick: u64) {
        self.state.pane = SheetPane::Left;
        self.state.picker = None;
        self.pulling = None;
        self.state.tile = self.tile();
        self.changed = tick;
        self.card = tick;
    }

    /// The selected tile, always one that exists.
    fn tile(&self) -> usize {
        self.state.tile.min(Num::ALL.len() - 1)
    }

    fn select(&mut self, tile: usize, tick: u64) {
        if tile != self.state.tile {
            self.state.tile = tile;
            self.changed = tick;
        }
    }

    /// Handles a key; true when it was the sheet's. The tab's own keys (`q` `e`,
    /// the digits, tab and esc) go on to the deck.
    pub fn key(&mut self, st: &mut AppState, code: KeyCode) -> bool {
        let tick = st.tick;
        let tile = self.tile();
        let b = block_of(Num::ALL[tile]);
        let here = span(b);
        let col = tile - here.start;
        // The same column in another block, clamped to its row.
        let at = |b: usize| {
            let s = span(b);
            s.start + col.min(s.len() - 1)
        };
        match (self.state.pane, code) {
            (SheetPane::Left, KeyCode::Left | KeyCode::Char('h')) => {
                if col > 0 {
                    self.select(tile - 1, tick);
                }
            }
            (SheetPane::Left, KeyCode::Right | KeyCode::Char('l')) => {
                if tile + 1 < here.end {
                    self.select(tile + 1, tick);
                } else {
                    self.state.pane = SheetPane::Ledger;
                }
            }
            (SheetPane::Left, KeyCode::Up | KeyCode::Char('k')) => {
                if b > 0 {
                    self.select(at(b - 1), tick);
                }
            }
            (SheetPane::Left, KeyCode::Down | KeyCode::Char('j')) => {
                if b + 1 < BLOCKS.len() {
                    self.select(at(b + 1), tick);
                }
            }
            // A point buys straight away; the refusal lands in the strip.
            (SheetPane::Left, KeyCode::Char(' ') | KeyCode::Enter) => {
                if let Num::Attribute(s) = Num::ALL[tile] {
                    let result = perform(st, Action::BuyAttribute(s));
                    report(st, result);
                }
            }
            (SheetPane::Ledger, _) => return self.ledger_key(st, code),
            _ => return false,
        }
        true
    }

    /// Draws the tab into `body`: the left pane's three blocks of tiles, the rule, the
    /// selected tile's readout typing out under it, the rule between the panes, and the
    /// right pane's ledger and card.
    pub fn draw(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        let x0 = body.x + 1;
        let right = (x0 + PANE_W).min(body.right());
        let focused = self.state.pane == SheetPane::Left;
        let readings: Vec<Reading> = Num::ALL.iter().map(|&n| Reading::of(st, n)).collect();
        let in_body = |dy: u16| (body.y + dy < body.bottom()).then_some(body.y + dy);
        // The pane is solid, so the map's ghost stays out of the gaps between tiles.
        let pane_w = RULE_X.min(body.width);
        solid(buf, Rect::new(body.x, body.y, pane_w, body.height));

        for (b, &(title, dy, w)) in BLOCKS.iter().enumerate() {
            let Some(y) = in_body(dy) else {
                continue;
            };
            let tail = match b {
                // The attribute points left, always: `0 AP` once they are spent.
                0 => vec![
                    (
                        st.character.attribute_points.to_string(),
                        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                    ),
                    (" AP".to_string(), Style::default().fg(DIM)),
                ],
                1 => vec![(
                    "from gear & chrome".to_string(),
                    Style::default().fg(FAINTER),
                )],
                _ => Vec::new(),
            };
            header(buf, (x0, right), y, title, &tail, focused);
            let ty = y + TILES_DY;
            for (k, i) in span(b).enumerate() {
                let x = x0 + k as u16 * w;
                if ty + 2 < body.bottom() && x + w <= right {
                    let sel = i == self.tile();
                    self.draw_tile(st, &readings[i], Rect::new(x, ty, w, 3), sel, buf);
                }
            }
        }

        if let Some(y) = in_body(READOUT_RULE) {
            put(
                buf,
                x0,
                y,
                &"─".repeat(PANE_W as usize),
                Style::default().fg(RULE),
                right,
            );
        }
        if let Some(y) = in_body(READOUT_Y) {
            let area = Rect::new(x0, y, right - x0, body.bottom() - y);
            let (w, h) = (area.width as usize, area.height as usize);
            let lines = Readout::of(st, &readings[self.tile()], w).fit(w, h);
            let p = st.tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
            put_paragraph(
                buf,
                area,
                reveal(&lines, p, (st.tick / 4).is_multiple_of(2)),
            );
        }

        self.draw_right(st, body, buf);

        // Amber while chrome can change: in the doc's chair, or free.
        let rule_x = body.x + RULE_X;
        let rule = match crate::clinic::operating(st) {
            Operating::Chair { .. } | Operating::Free => AMBER,
            Operating::Locked | Operating::Gone { .. } => RULE,
        };
        if rule_x < body.right() {
            for y in body.y..body.bottom() {
                buf[(rule_x, y)].set_char('║').set_fg(rule);
            }
        }
    }

    /// One number's tile: its label set in the top edge, the value inside, what gear,
    /// chrome and talents add set in the bottom edge. The selected tile's edge is cyan
    /// and its inside lit while the pane has the focus, dim while it does not.
    fn draw_tile(&self, st: &AppState, r: &Reading, t: Rect, sel: bool, buf: &mut Buffer) {
        let (x, y, w, right) = (t.x, t.y, t.width, t.right());
        let inner = right.saturating_sub(1);
        let focused = self.state.pane == SheetPane::Left;
        let on = sel && focused;
        let edge = Style::default().fg(match (sel, focused) {
            (false, _) => RULE,
            (true, true) => CYAN,
            (true, false) => DIM,
        });

        let label = r.num.label();
        let run = "─".repeat((w as usize).saturating_sub(label.chars().count() + 4));
        put(buf, x, y, &format!("┌ {label} {run}┐"), edge, right);
        let label_style = if on {
            Style::default().fg(lit(st.tick))
        } else {
            Style::default().fg(match r.num {
                Num::Attribute(s) => attribute_colour(s),
                _ => DIM,
            })
        };
        put(
            buf,
            x + 2,
            y,
            label,
            label_style.add_modifier(Modifier::BOLD),
            inner,
        );

        put(buf, x, y + 1, "│", edge, right);
        put(buf, inner, y + 1, "│", edge, right);
        let mut style = Style::default().fg(r.colour()).add_modifier(Modifier::BOLD);
        if on {
            for cx in x + 1..inner {
                buf[(cx, y + 1)].set_char(' ').set_bg(BAR_BG);
            }
            style = style.bg(BAR_BG);
        }
        let value = r.value.to_string();
        let room = w.saturating_sub(2);
        let vx = x + 1 + room.saturating_sub(value.chars().count() as u16) / 2;
        put(buf, vx, y + 1, &value, style, inner);

        put(
            buf,
            x,
            y + 2,
            &format!("└{}┘", "─".repeat(room as usize)),
            edge,
            right,
        );
        if let Some((text, colour)) = r.modifier() {
            let mut style = Style::default().fg(colour);
            if on {
                style = style.add_modifier(Modifier::BOLD);
            }
            put(buf, x + 2, y + 2, " ", edge, inner);
            put(buf, x + 3, y + 2, &text, style, inner);
            put(
                buf,
                x + 3 + text.chars().count() as u16,
                y + 2,
                " ",
                edge,
                inner,
            );
        }
    }

    /// The hint row's text, by where the cursor is.
    #[cfg(test)]
    pub fn hints(&self, st: &AppState) -> String {
        self.hint_line(st).to_string()
    }

    /// The hint row, by where the cursor is: dim, and what the lock refuses dimmer.
    pub fn hint_line(&self, st: &AppState) -> Line<'static> {
        match self.state.pane {
            SheetPane::Left => Line::from(Span::styled(
                "←↑↓→ select · space allocates attribute · → over to the chrome · Q/E tabs · tab close",
                Style::default().fg(DIM),
            )),
            SheetPane::Ledger => self.ledger_hints(st),
        }
    }
}

/// A block's header: its title, cyan while the pane has the focus, a rule, and the
/// tail set at the right end.
fn header(
    buf: &mut Buffer,
    (x0, right): (u16, u16),
    y: u16,
    title: &str,
    tail: &[(String, Style)],
    lit: bool,
) {
    marked_header(buf, (x0, right), y, title, &[], tail, lit);
}

/// `header` with a mark straight after the title, before the rule.
fn marked_header(
    buf: &mut Buffer,
    (x0, right): (u16, u16),
    y: u16,
    title: &str,
    mark: &[(String, Style)],
    tail: &[(String, Style)],
    lit: bool,
) {
    let title_style = Style::default()
        .fg(if lit { CYAN } else { DIM })
        .add_modifier(Modifier::BOLD);
    put(buf, x0, y, title, title_style, right);
    let mut x = x0 + title.chars().count() as u16;
    for (text, style) in mark {
        put(buf, x, y, text, *style, right);
        x += text.chars().count() as u16;
    }
    let tail_w: u16 = tail
        .iter()
        .map(|(t, _)| t.chars().count() as u16)
        .sum::<u16>()
        + u16::from(!tail.is_empty());
    let start = x + 1;
    let end = right.saturating_sub(tail_w);
    if start < end {
        put(
            buf,
            start,
            y,
            &"─".repeat((end - start) as usize),
            Style::default().fg(RULE),
            end,
        );
    }
    let mut x = end + 1;
    for (text, style) in tail {
        put(buf, x, y, text, *style, right);
        x += text.chars().count() as u16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::render::map::{MapRef, TileMap};
    use crate::screens::create::attribute_colour;
    use crate::screens::deck::{Deck, REVEAL_TICKS};
    use crate::screens::testutil::key;
    use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT};
    use citygen::{GenParams, Generator, SizePreset};
    use game::{Attribute, Build, ItemId, Num, Site, Slot};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// The deck's body at 100×30, and at 120×40.
    const BODY: Rect = Rect::new(1, 5, 98, 22);
    const WIDE: Rect = Rect::new(1, 10, 118, 27);

    fn state() -> AppState {
        AppState::new()
    }

    /// A city to stand in, and the player on its street.
    fn walking() -> AppState {
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

    fn tile_of(n: Num) -> usize {
        Num::ALL.iter().position(|&m| m == n).unwrap()
    }

    /// `n` cells of row `y` from column `x`.
    fn cells(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
    }

    /// The tick at which the readout has typed out.
    fn settle(s: &Sheet, st: &mut AppState) {
        st.tick = s.changed + REVEAL_TICKS + 1;
    }

    fn drawn(s: &Sheet, st: &AppState, body: Rect) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, body.right() + 1, body.bottom() + 3));
        s.draw(st, body, &mut buf);
        buf
    }

    /// The readout's rows, the left pane's columns only.
    fn readout(buf: &Buffer, body: Rect) -> String {
        (body.y + READOUT_Y..body.bottom())
            .map(|y| cells(buf, body.x + 1, y, 43).trim_end().to_string() + "\n")
            .collect()
    }

    fn press(s: &mut Sheet, st: &mut AppState, code: KeyCode) -> usize {
        assert!(s.key(st, code), "{code:?} is the sheet's");
        s.state.tile
    }

    /// Nine pieces in Vex: her optics and eight more.
    fn nine_pieces(st: &mut AppState) {
        for (site, id) in [
            (Site::Neural, "neural_port"),
            (Site::Ears, "damper_cochlea"),
            (Site::Jaw, "rebreather"),
            (Site::Spine, "hardback_brace"),
            (Site::Back, "drone_cradle"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "smart_ink"),
            (Site::LeftArm, "piston_arm"),
        ] {
            st.character.install(site, ItemId::new(id), None);
        }
        assert_eq!(st.character.pieces(&st.content), 9);
    }

    #[test]
    fn left_pane_draws_its_tiles() {
        let mut st = state();
        st.character.attribute_points = 2;
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        st.tick = 60;
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| d.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let body: String = (BODY.y..BODY.bottom())
            .map(|y| cells(buf, BODY.x, y, BODY.width) + "\n")
            .collect();
        for want in [
            "ATTRIBUTES",
            "2 AP",
            "┌ BOD",
            "SECONDARY",
            "from gear & chrome",
            "┌ ARM",
            "RESISTANCES",
            "┌ EMP",
        ] {
            assert!(body.contains(want), "{want}: {body}");
        }
        // The mock's columns: the headers from x 2, their tails ending at x 44; a
        // blank row under each header, then its tiles.
        assert_eq!(cells(buf, 2, 5, 10), "ATTRIBUTES");
        assert_eq!(cells(buf, 41, 5, 4), "2 AP");
        assert_eq!(cells(buf, 2, 7, 16), "┌ BOD ─┐┌ REF ─┐");
        assert_eq!(cells(buf, 2, 11, 9), "SECONDARY");
        assert_eq!(cells(buf, 27, 11, 18), "from gear & chrome");
        assert_eq!(
            cells(buf, 2, 13, 42),
            "┌ HP ─┐┌ FOC ┐┌ ARM ┐┌ SGT ┐┌ CAR ┐┌ MOV ┐"
        );
        assert_eq!(cells(buf, 2, 17, 11), "RESISTANCES");
        assert_eq!(cells(buf, 2, 19, 14), "┌ KIN ┐┌ THM ┐");
        // Blank: under each header and between the blocks; the readout's rule sits right
        // under the last tiles, and the readout has the four rows under it.
        for y in [6, 10, 12, 16, 18] {
            assert_eq!(cells(buf, 2, y, 43).trim(), "", "row {y}");
        }
        assert_eq!(cells(buf, 2, 21, 7), "└─────┘", "KIN's bottom edge");
        assert_eq!(cells(buf, 2, 22, 43), "─".repeat(43), "the rule");
        assert_eq!(BODY.bottom() - (BODY.y + READOUT_Y), 4);
        assert_eq!(cells(buf, 2, 23, 4), "BODY", "the readout under the rule");
        // The deck's +1 net in the NET tile's bottom edge, the jacket's +5 in HP's;
        // with no body SGT moved nothing, so its edge is bare.
        assert_eq!(cells(buf, 34, 9, 8), "└─ +1 ─┘");
        assert_eq!(buf[(37, 9)].fg, GREEN);
        assert_eq!(cells(buf, 2, 15, 7), "└─ +5 ┘");
        assert_eq!(cells(buf, 23, 15, 7), "└─────┘");
        // BODY, selected on the focused pane: cyan edge, the bar behind its value.
        assert_eq!(buf[(2, 7)].fg, CYAN);
        assert_eq!(buf[(5, 8)].bg, BAR_BG);
        assert_eq!(buf[(10, 7)].fg, RULE, "REF's edge, not selected");
        assert_eq!(
            buf[(12, 7)].fg,
            attribute_colour(Attribute::Reflex),
            "REF in its colour"
        );
        // Nothing of the left pane past body.x + 44; the rule stands at body.x + 45.
        for y in BODY.y..BODY.bottom() {
            assert_eq!(buf[(BODY.x + 44, y)].symbol(), " ", "y {y}");
            assert_eq!(buf[(BODY.x + 45, y)].symbol(), "║", "y {y}");
        }
    }

    #[test]
    fn the_header_always_says_what_points_are_left() {
        let mut st = state();
        for (points, tail) in [(0, "0 AP"), (1, "1 AP"), (12, "12 AP")] {
            st.character.attribute_points = points;
            let s = Sheet::new(0);
            settle(&s, &mut st);
            let buf = drawn(&s, &st, BODY);
            let w = tail.chars().count() as u16;
            assert_eq!(cells(&buf, 45 - w, BODY.y, w), tail);
            // The number bold TEXT, the word DIM.
            let n = &buf[(45 - w, BODY.y)];
            assert_eq!((n.fg, n.modifier.contains(Modifier::BOLD)), (TEXT, true));
            assert_eq!(buf[(43, BODY.y)].fg, DIM);
            assert_eq!(buf[(44, BODY.y)].fg, DIM);
            assert_eq!(buf[(45 - w - 2, BODY.y)].symbol(), "─");
            // The next point's cost only while there are points to spend.
            let text = readout(&buf, BODY);
            assert_eq!(text.contains("next "), points > 0, "{points}: {text}");
        }
    }

    #[test]
    fn keys_walk_the_tiles() {
        let mut st = state();
        st.character.attribute_points = 3;
        let mut s = Sheet::new(0);
        assert_eq!(s.state.tile, 0, "BODY the first time");
        for _ in 0..3 {
            press(&mut s, &mut st, KeyCode::Right);
        }
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Right),
            tile_of(Num::Attribute(Attribute::Net))
        );
        assert_eq!(s.state.pane, SheetPane::Left);
        press(&mut s, &mut st, KeyCode::Right);
        assert_eq!(
            s.state.pane,
            SheetPane::Ledger,
            "past NET, over to the chrome"
        );
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Left),
            tile_of(Num::Attribute(Attribute::Net))
        );
        assert_eq!(s.state.pane, SheetPane::Left, "back to the tile it had");
        for _ in 0..3 {
            press(&mut s, &mut st, KeyCode::Char('h'));
        }
        assert_eq!(s.state.tile, tile_of(Num::Attribute(Attribute::Reflex)));
        assert_eq!(press(&mut s, &mut st, KeyCode::Down), tile_of(Num::Focus));
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Char('j')),
            tile_of(Num::Resist(game::Harm::Thermal))
        );
        st.tick = 9;
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Down),
            tile_of(Num::Resist(game::Harm::Thermal)),
            "the last block stops"
        );
        assert_eq!(s.changed, 0, "a stop is not a move");
        // Up from MOV, the sixth column, clamps to NET; down from NET keeps column 4.
        s.state.tile = tile_of(Num::Move);
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Up),
            tile_of(Num::Attribute(Attribute::Net))
        );
        assert_eq!(s.changed, 9);
        assert_eq!(press(&mut s, &mut st, KeyCode::Down), tile_of(Num::Carry));
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Char('k')),
            tile_of(Num::Attribute(Attribute::Net))
        );
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Up),
            tile_of(Num::Attribute(Attribute::Net))
        );

        // Space on BODY buys a point; on ARM it does nothing.
        s.state.tile = 0;
        let cost = Build::from_character(&st.content, &st.character)
            .attribute_cost(&st.content, Attribute::Body);
        let (body, points) = (st.character.attributes.body, st.character.attribute_points);
        press(&mut s, &mut st, KeyCode::Char(' '));
        assert_eq!(
            (st.character.attributes.body, st.character.attribute_points),
            (body + 1, points - cost)
        );
        assert!(
            st.log
                .latest()
                .is_some_and(|e| e.text == format!("BODY {}", body + 1))
        );
        s.state.tile = tile_of(Num::Armor);
        let before = (
            st.character.clone(),
            st.log.latest().map(|e| e.text.clone()),
        );
        press(&mut s, &mut st, KeyCode::Char(' '));
        assert_eq!(
            (
                st.character.clone(),
                st.log.latest().map(|e| e.text.clone())
            ),
            before
        );
        // The deck keeps its own keys.
        for code in [
            KeyCode::Esc,
            KeyCode::Tab,
            KeyCode::Char('q'),
            KeyCode::Char('e'),
            KeyCode::Char('2'),
        ] {
            assert!(!s.key(&mut st, code), "{code:?}");
        }
    }

    #[test]
    fn the_readout_names_a_drone_by_its_kind() {
        let mut st = state();
        let mut cradle = st
            .character
            .new_copy(&st.content, &ItemId::new("drone_cradle"));
        let mule = ItemId::new("mule_drone");
        let copy = st.character.new_copy(&st.content, &mule);
        cradle.fitted[0] = Some(game::Stack {
            item: mule,
            qty: 1,
            copy: Some(copy),
        });
        st.character
            .install(Site::Back, ItemId::new("drone_cradle"), Some(cradle));
        let mut s = Sheet::new(0);
        s.state.tile = tile_of(Num::Carry);
        settle(&s, &mut st);
        let text = readout(&drawn(&s, &st, BODY), BODY);
        assert!(text.contains("drone +8 Mule drone"), "{text}");
        assert!(!text.contains("chrome +8"), "{text}");
    }

    #[test]
    fn readout_names_sources() {
        let mut st = state();
        st.character.attribute_points = 3;
        let arm = ItemId::new("piston_arm");
        st.character.install(Site::LeftArm, arm, None);
        let mut s = Sheet::new(0);
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        let n = Build::from_character(&st.content, &st.character)
            .attribute_cost(&st.content, Attribute::Body);
        let next = format!("next {n} pt{}", if n == 1 { "" } else { "s" });
        for want in [
            "BODY 4 · 5 with chrome",
            &next,
            "feeds",
            "◇ Athletics",
            "chrome +1 Piston arm",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        assert!(
            text.lines().next().unwrap().ends_with(&next),
            "right-aligned: {text}"
        );
        // Four rows at 100×30: the blurb's first line stays, marked cut.
        assert_eq!(text.lines().count(), 4, "{text}");
        assert!(text.lines().nth(1).unwrap().ends_with('…'), "{text}");
        assert_eq!(
            buf[(2, BODY.y + READOUT_Y)].fg,
            attribute_colour(Attribute::Body)
        );
        assert_eq!(buf[(BODY.x + 43, BODY.y + READOUT_Y)].fg, AMBER);

        // EMP, with nine pieces in: the tile and the readout say what they cost.
        nine_pieces(&mut st);
        s.state.tile = tile_of(Num::Resist(game::Harm::Emp));
        s.changed = st.tick;
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        assert!(text.starts_with("EMP -18"), "{text}");
        assert!(text.contains("chrome -18 · nine pieces"), "{text}");
        assert!(!text.contains("feeds"), "{text}");
        assert_eq!(cells(&buf, 23, 20, 7), "│ -18 │");
        assert_eq!(buf[(25, 20)].fg, MAGENTA);

        // SIGHT: the number the map sees by, where you stand.
        let mut st = walking();
        let mut s = Sheet::new(0);
        s.state.tile = tile_of(Num::Sight);
        settle(&s, &mut st);
        let p = st.player.as_ref().unwrap();
        let city = st.city.as_ref().unwrap();
        let map = MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            p.place,
            st.minute_of_day(),
        )
        .unwrap();
        // At night: the street's radius, the optics' four and Low-light's two.
        assert!(citygen::lighting::lamps_on(st.minute_of_day()));
        let here = p.radius_at(map.daylight());
        let radius = (here + st.sight_bonus()).max(1);
        assert_eq!(radius, here + 6);
        let buf = drawn(&s, &st, BODY);
        assert_eq!(
            cells(&buf, 23, 14, 7).trim_matches(['│', ' ']),
            radius.to_string()
        );
        assert_eq!(cells(&buf, 23, 15, 7), "└─ +6 ┘");
        let text = readout(&buf, BODY);
        assert!(
            text.starts_with(&format!("SIGHT {here} · {radius} with chrome")),
            "{text}"
        );
        assert!(text.contains("chrome +4 Nightglass optics\n"), "{text}");
        assert!(
            text.contains("chrome +2 Low-light · Nightglass optics"),
            "{text}"
        );
        // By day Low-light does nothing, and says nothing.
        while citygen::lighting::lamps_on(st.minute_of_day()) {
            st.tick += 900;
        }
        s.changed = st.tick - REVEAL_TICKS - 1;
        let p = st.player.as_ref().unwrap();
        let city = st.city.as_ref().unwrap();
        let map = MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            p.place,
            st.minute_of_day(),
        )
        .unwrap();
        let here = p.radius_at(map.daylight());
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        assert!(
            text.starts_with(&format!("SIGHT {here} · {} with chrome", here + 4)),
            "{text}"
        );
        assert!(!text.contains("Low-light"), "{text}");
        assert_eq!(cells(&buf, 23, 15, 7), "└─ +4 ┘");
        // No body: no sight, no edge, and the readout says why.
        st.player = None;
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, 23, 14, 7), "│  0  │");
        assert_eq!(cells(&buf, 23, 15, 7), "└─────┘");
        let text = readout(&buf, BODY);
        assert!(text.starts_with("SIGHT 0\n"), "{text}");
        assert!(text.contains("chrome +4 Nightglass optics"), "{text}");
        assert!(text.contains("no body: 0"), "{text}");
    }

    /// The readout's row that starts with `lead`, as drawn.
    fn row_of(buf: &Buffer, body: Rect, lead: &str) -> u16 {
        (body.y + READOUT_Y..body.bottom())
            .find(|&y| cells(buf, body.x + 1, y, 43).starts_with(lead))
            .unwrap_or_else(|| panic!("no row starts with {lead}"))
    }

    #[test]
    fn the_edge_is_what_the_value_moved_and_the_readout_says_why() {
        // BODY 8, a Wrecker arm and a Piston arm: three more, but BODY stops at 10.
        let mut st = state();
        st.character.attributes.body = 8;
        st.character
            .install(Site::LeftArm, ItemId::new("wrecker_arm"), None);
        st.character
            .install(Site::RightArm, ItemId::new("piston_arm"), None);
        let s = Sheet::new(0);
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, 2, 8, 8), "│  10  │");
        assert_eq!(cells(&buf, 2, 9, 8), "└─ +2 ─┘");
        let text = readout(&buf, BODY);
        for want in [
            "BODY 8 · 10 with chrome",
            "chrome +2 Wrecker arm · +1 Piston arm",
            "capped at 10",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        let y = row_of(&buf, BODY, "capped at 10");
        assert_eq!(buf[(2, y)].fg, FAINTER);

        // MOV in a construct: your avatar walks BASE_MOVE whatever your spine does.
        let mut st = walking();
        st.character
            .install(Site::Spine, ItemId::new("redline_spine"), None);
        let b = st.city.as_ref().unwrap().buildings[0].id;
        st.player.as_mut().unwrap().place = Place::Construct(b);
        let mut s = Sheet::new(0);
        s.state.tile = tile_of(Num::Move);
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        let mov = 2 + 5 * 7;
        assert_eq!(cells(&buf, mov, 14, 7), "│  4  │");
        assert_eq!(
            cells(&buf, mov, 15, 7),
            "└─────┘",
            "no edge: it moved nothing"
        );
        assert_eq!(buf[(mov + 3, 14)].fg, TEXT);
        let text = readout(&buf, BODY);
        assert!(text.starts_with("MOVE 4\n"), "{text}");
        assert!(text.contains("chrome +1 Redline spine"), "{text}");
        assert!(text.contains("in a construct: 4"), "{text}");
        // On foot the spine counts, and nothing needs saying.
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Street);
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, mov, 15, 7), "└─ +1 ┘");
        let text = readout(&buf, BODY);
        assert!(text.starts_with("MOVE 4 · 5 with chrome"), "{text}");
        assert!(!text.contains("in a construct:"), "{text}");
    }

    #[test]
    fn line_one_fits_beside_the_cost_and_no_source_is_lost() {
        // COOL from the mirrorshades and the Hushcan on the Ratchet, points unspent.
        let mut st = state();
        st.character.attribute_points = 3;
        st.character
            .wear(Slot::Eyes, ItemId::new("mirrorshades"), None);
        let hush = ItemId::new("hushcan");
        let copy = st.character.new_copy(&st.content, &hush);
        let content = st.content.clone();
        st.character
            .host_copy_mut(&content, game::Host::Worn(Slot::Primary))
            .unwrap()
            .fitted[0] = Some(game::Stack {
            item: hush,
            qty: 1,
            copy: Some(copy),
        });
        let mut s = Sheet::new(0);
        s.state.tile = tile_of(Num::Attribute(Attribute::Cool));
        settle(&s, &mut st);
        // Six rows of readout: room for what it feeds, not for the whole blurb.
        let body = Rect::new(1, 5, 98, READOUT_Y + 6);
        let buf = drawn(&s, &st, body);
        let text = readout(&buf, body);
        let n = Build::from_character(&st.content, &st.character)
            .attribute_cost(&st.content, Attribute::Cool);
        let next = format!("next {n} pt{}", if n == 1 { "" } else { "s" });
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("COOL 5 · 7 in all"), "{text}");
        assert!(
            first.ends_with(&next),
            "one row, the cost at the right: {text}"
        );
        assert_eq!(buf[(body.x + 43, body.y + READOUT_Y)].fg, AMBER);
        for want in ["feeds", "gear +1 Mirrorshades", "hardware +1 Hushcan"] {
            assert!(text.contains(want), "{want}: {text}");
        }
        // The blurb gave way to make room, and says so.
        assert!(text.lines().nth(1).unwrap().ends_with('…'), "{text}");

        // On the real 100×30 body, four rows: line one with its cost, every source.
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        assert_eq!(text.lines().filter(|l| !l.is_empty()).count(), 4, "{text}");
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("COOL 5 · 7 in all"), "{text}");
        assert!(first.ends_with(&next), "{text}");
        for want in ["gear +1 Mirrorshades", "hardware +1 Hushcan"] {
            assert!(text.contains(want), "{want}: {text}");
        }
    }

    /// REFLEX `base` with the Monowire shiv worn, the Redline spine in and Quick draw at
    /// 2: three more, capped at 10 from 8 up, points unspent.
    fn crowded_reflex(base: u8) -> (AppState, Sheet) {
        let mut st = state();
        st.character.attribute_points = 3;
        st.character.attributes.reflex = base;
        st.character
            .wear(Slot::Secondary, ItemId::new("shiv_blade"), None);
        st.character
            .install(Site::Spine, ItemId::new("redline_spine"), None);
        st.character
            .talents
            .insert(game::TalentId::new("quick_draw"), 2);
        let mut s = Sheet::new(0);
        s.state.tile = tile_of(Num::Attribute(Attribute::Reflex));
        settle(&s, &mut st);
        (st, s)
    }

    #[test]
    fn a_crowded_readout_folds_its_note_into_line_one() {
        let (st, s) = crowded_reflex(9);
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        let first = text.lines().next().unwrap();
        assert_eq!(first, "REFLEX 9 · 10 in all, capped", "{text}");
        for want in [
            "gear +1 Monowire shiv",
            "chrome +1 Redline spine",
            "talents +1",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        assert!(!text.contains('…'), "nothing cut: {text}");
        // The folded note in the note's own colour.
        let x = BODY.x + 1 + "REFLEX 9 · 10 in all, ".chars().count() as u16;
        assert_eq!(cells(&buf, x, BODY.y + READOUT_Y, 6), "capped");
        assert_eq!(buf[(x, BODY.y + READOUT_Y)].fg, FAINTER);
        // With room, the note keeps its own line.
        let wide = drawn(&s, &st, WIDE);
        let text = readout(&wide, WIDE);
        assert!(text.starts_with("REFLEX 9 · 10 in all\n"), "{text}");
        assert!(text.contains("\ncapped at 10\n"), "{text}");
    }

    #[test]
    fn a_more_crowded_readout_flows_its_sources() {
        // REFLEX 7, under the cap a point buys to, and the Redeye sight on the Ratchet's
        // rail as well: four kinds, the cap, and the next point's cost on line one.
        let (mut st, s) = crowded_reflex(7);
        let eye = ItemId::new("redeye");
        let copy = st.character.new_copy(&st.content, &eye);
        let content = st.content.clone();
        st.character
            .host_copy_mut(&content, game::Host::Worn(Slot::Primary))
            .unwrap()
            .fitted[1] = Some(game::Stack {
            item: eye,
            qty: 1,
            copy: Some(copy),
        });
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        let n = Build::from_character(&st.content, &st.character)
            .attribute_cost(&st.content, Attribute::Reflex);
        let next = format!("next {n} pt{}", if n == 1 { "" } else { "s" });
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("REFLEX 7 · 10 in all, capped"), "{text}");
        assert!(
            first.ends_with(&next),
            "the cost still at the right: {text}"
        );
        for want in [
            "gear +1 Monowire shiv",
            "hardware +1 Redeye sight",
            "chrome +1 Redline spine · talents +1",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        assert!(!text.contains('…'), "nothing cut: {text}");
    }

    #[test]
    fn a_readout_with_no_room_left_marks_its_last_line_cut() {
        let (st, _) = crowded_reflex(9);
        let r = Reading::of(&st, Num::Attribute(Attribute::Reflex));
        let shown = |height: usize| -> Vec<String> {
            let lines = Readout::of(&st, &r, 43).fit(43, height);
            lines.iter().map(|l| l.to_string()).collect()
        };
        // Three rows: the note folded, the sources flowed onto two lines, nothing cut.
        assert_eq!(
            shown(3),
            [
                "REFLEX 9 · 10 in all, capped",
                "gear +1 Monowire shiv",
                "chrome +1 Redline spine · talents +1",
            ]
        );
        // Two: the last line that fits says the rest was cut.
        assert_eq!(
            shown(2),
            ["REFLEX 9 · 10 in all, capped", "gear +1 Monowire shiv…"]
        );
        assert_eq!(shown(1), ["REFLEX 9 · 10 in all, capped…"]);
        // A full-width line (43 of 43) gives up its last cell to the mark.
        let line = Line::from(vec![
            Span::styled("a".repeat(30), Style::default().fg(DIM)),
            Span::styled(
                " ".to_string() + &"b".repeat(12),
                Style::default().fg(GREEN),
            ),
        ]);
        let cut = marked_cut(&line, 43);
        assert_eq!(
            cut.to_string(),
            format!("{} {}…", "a".repeat(30), "b".repeat(11))
        );
        assert_eq!(cut.spans.last().unwrap().style.fg, Some(GREEN));
    }

    #[test]
    fn the_readout_types_out_on_each_change() {
        let mut st = state();
        let mut s = Sheet::new(0);
        settle(&s, &mut st);
        let tick = st.tick;
        press(&mut s, &mut st, KeyCode::Right);
        assert_eq!(s.changed, tick);
        let text = readout(&drawn(&s, &st, BODY), BODY);
        assert!(!text.contains("REFLEX"), "nothing typed yet: {text}");
        st.tick += REVEAL_TICKS;
        let text = readout(&drawn(&s, &st, BODY), BODY);
        assert!(text.starts_with("REFLEX"), "{text}");
    }

    #[test]
    fn the_ledgers_focus_dims_the_tile() {
        let mut st = state();
        let mut s = Sheet::new(0);
        s.state.pane = SheetPane::Ledger;
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        assert_eq!(buf[(2, 7)].fg, DIM, "BODY keeps its place, dim");
        assert_ne!(buf[(5, 8)].bg, BAR_BG);
        assert_eq!(buf[(2, 5)].fg, DIM, "the headers go dim");
        assert!(s.hints(&st).contains("← attributes"));
        s.state.pane = SheetPane::Left;
        assert_eq!(
            s.hints(&st),
            "←↑↓→ select · space allocates attribute · → over to the chrome · Q/E tabs · tab close"
        );
    }

    #[test]
    fn nothing_spills_past_the_left_pane() {
        let mut st = walking();
        nine_pieces(&mut st);
        st.character.attribute_points = 12;
        st.character
            .talents
            .insert(game::TalentId::new("thick_skin"), 3);
        let area = Rect::new(0, 0, 120, 40);
        for body in [BODY, WIDE] {
            for tile in 0..Num::ALL.len() {
                for pane in [SheetPane::Left, SheetPane::Ledger] {
                    let mut s = Sheet::new(0);
                    s.state = SheetState {
                        pane,
                        tile,
                        ..Default::default()
                    };
                    settle(&s, &mut st);
                    let mut buf = Buffer::empty(area);
                    for c in &mut buf.content {
                        c.set_char('X');
                    }
                    s.draw(&st, body, &mut buf);
                    for y in area.y..area.bottom() {
                        for x in area.x..area.right() {
                            let inside = body.contains((x, y).into());
                            let sym = buf[(x, y)].symbol();
                            if !inside {
                                assert_eq!(sym, "X", "{body:?} tile {tile} ({x},{y})");
                            } else if x == body.x + 45 {
                                assert_eq!(sym, "║", "{body:?} tile {tile} ({x},{y})");
                            }
                        }
                    }
                    // The pane's last column stays clear: nothing runs into the rule.
                    // The pane is solid: no ghost between the blocks.
                    for y in body.y..body.bottom() {
                        assert_eq!(
                            buf[(body.x + 44, y)].symbol(),
                            " ",
                            "{body:?} tile {tile} y {y}"
                        );
                    }
                    for dy in [1, 5, 7] {
                        assert_eq!(buf[(body.x, body.y + dy)].symbol(), " ");
                    }
                    // Everything the number comes from is drawn, and why it doesn't add up.
                    let text = readout(&buf, body);
                    let r = Reading::of(&st, Num::ALL[tile]);
                    for part in Readout::of(&st, &r, 43).says() {
                        assert!(
                            text.contains(&part),
                            "{body:?} tile {tile}: {part:?} in {text}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn at_120x40_the_pane_keeps_its_width_and_the_readout_its_place() {
        let mut st = state();
        let s = Sheet::new(0);
        settle(&s, &mut st);
        let wide = drawn(&s, &st, WIDE);
        assert_eq!(cells(&wide, 2, WIDE.y, 10), "ATTRIBUTES");
        assert_eq!(cells(&wide, 2, WIDE.y + 1, 43).trim(), "");
        assert_eq!(cells(&wide, 2, WIDE.y + 2, 8), "┌ BOD ─┐");
        // Room for the whole readout: line one, the blurb, a blank row, what it feeds.
        let text = readout(&wide, WIDE);
        assert!(text.contains("feeds"), "{text}");
        assert!(!text.contains('…'), "{text}");
        assert!(readout(&wide, WIDE).starts_with("BODY 4"));
        assert_eq!(wide[(WIDE.x + 45, WIDE.bottom() - 1)].symbol(), "║");
    }
}
