//! STATS, the character sheet: on the left the stats you buy, the secondary numbers
//! gear and chrome move and the resistances, as rack tiles with a readout of the one
//! selected; on the right the ledger of the chrome in you.

use crate::actions::{perform, report};
use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use crate::screens::create::{stat_colour, wrap};
use crate::screens::deck::{REVEAL_TICKS, cut, lit};
use crate::screens::popup::wrapped_rows;
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT, put, put_paragraph, reveal, solid,
};
use citygen::{Place, StratumKind};
use crossterm::event::KeyCode;
use game::{Action, Build, Contrib, Num, Source};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::ops::Range;

/// A bonus.
pub(crate) const GOOD: Color = Color::Rgb(0x50, 0xe6, 0x78);
/// What a choice would cost.
pub(crate) const AMBER: Color = Color::Rgb(0xff, 0xc8, 0x50);

/// Columns of the left pane, from one in from the body's edge; one more stays clear
/// before the rule.
const PANE_W: u16 = 43;
/// The rule between the panes, from the body's left edge.
const RULE_X: u16 = 45;
/// The rule over the readout, and the readout under it, from the body's top.
const READOUT_RULE: u16 = 15;
const READOUT_Y: u16 = 16;

/// The left pane's blocks of tiles, in `Num::ALL` order: a header's title, the row
/// it sits on, and each tile's width.
const BLOCKS: [(&str, u16, u16); 3] =
    [("STATS", 0, 8), ("SECONDARY", 5, 7), ("RESISTANCES", 10, 7)];

/// The block a number's tile sits in.
fn block_of(n: Num) -> usize {
    match n {
        Num::Stat(_) => 0,
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
        1 => GOOD,
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
    note: Option<String>,
}

impl Reading {
    fn of(st: &AppState, num: Num) -> Reading {
        let (c, ch) = (&st.content, &st.character);
        let sources = ch.sources(c, num);
        let place = st.player.as_ref().map(|p| p.place);
        let value = match num {
            Num::Stat(s) => ch.stat(c, s) as i32,
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
            Num::Stat(s) => Some(ch.stats.get(s) as i32),
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
            (Num::Move, Some(_)) if matches!(place, Some(Place::Construct(_))) => {
                Some(format!("in a construct: {value}"))
            }
            (_, Some(sum)) if sum > value => Some(format!("capped at {value}")),
            (_, Some(_)) => Some(format!("at least {value}")),
            (Num::Sight, None) if added != 0 => Some(format!("no body: {value}")),
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

    /// The value's colour: a stat plain, a secondary number by its edge, a
    /// resistance by its sign.
    fn colour(&self) -> Color {
        match self.num {
            Num::Stat(_) => TEXT,
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

/// The kinds of source, in the order the readout lists them, each with its word.
const KINDS: [&str; 6] = [
    "gear", "hardware", "chrome", "talents", "category", "innate",
];

fn kind_of(from: &Contrib) -> usize {
    match from {
        Contrib::Gear(_) => 0,
        Contrib::Hardware(_) => 1,
        Contrib::Chrome(_) | Contrib::Pieces(_) => 2,
        Contrib::Talents => 3,
    }
}

/// The kind and the name of what grants an ability: an item is gear or chrome.
fn granted(st: &AppState, from: &Source) -> (usize, Option<String>) {
    let c = &st.content;
    match from {
        Source::Item(id) => {
            let item = c.item(id);
            let chrome = item.is_some_and(|i| i.implant.is_some());
            let name = item.map_or_else(|| id.to_string(), |i| i.name.clone());
            (if chrome { 2 } else { 0 }, Some(name))
        }
        Source::Talent(t) => (
            3,
            Some(
                c.talent(t)
                    .map_or_else(|| t.to_string(), |t| t.name.clone()),
            ),
        ),
        Source::Category(k) => (
            4,
            Some(
                c.category(k)
                    .map_or_else(|| k.to_string(), |k| k.name.clone()),
            ),
        ),
        Source::Innate => (5, None),
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

/// A number's readout, in parts so it can give way when rows run short: line one, the
/// blurb, what a stat feeds, and where the rest comes from with any note.
struct Readout {
    head: Line<'static>,
    blurb: Vec<Line<'static>>,
    feeds: Vec<Line<'static>>,
    parts: Vec<Line<'static>>,
}

impl Readout {
    /// The readout `width` columns wide.
    fn of(st: &AppState, r: &Reading, width: usize) -> Readout {
        let (c, ch) = (&st.content, &st.character);
        let bold = |colour: Color| Style::default().fg(colour).add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(DIM);
        let name = match r.num {
            Num::Stat(s) => stat_colour(s),
            _ => r.colour(),
        };
        // Its own value and the value with everything, while they differ and it has
        // one of its own to speak of; else the value.
        let mut head = vec![Span::styled(format!("{} ", r.num.name()), bold(name))];
        match r.own {
            Some(own) if own != r.value && r.num != Num::Armor => {
                let with = match r.kinds()[..] {
                    [k] => format!("with {}", KINDS[k]),
                    _ => "in all".to_string(),
                };
                head.push(Span::styled(own.to_string(), bold(TEXT)));
                head.push(Span::styled(format!(" · {} {with}", r.value), dim));
            }
            _ => head.push(Span::styled(r.value.to_string(), bold(r.colour()))),
        }
        // What the next point costs, at the right, while there are points to spend.
        if let Num::Stat(s) = r.num
            && ch.stat_points > 0
            && ch.stats.get(s) < c.creation.stat_cap
        {
            let n = Build::from_character(c, ch).stat_cost(c, s);
            let next = format!("next {n} pt{}", if n == 1 { "" } else { "s" });
            let used: usize = head.iter().map(|s| s.width()).sum();
            let pad = width.saturating_sub(used + next.chars().count()).max(2);
            head.push(Span::raw(" ".repeat(pad)));
            head.push(Span::styled(next, Style::default().fg(AMBER)));
        }

        let blurb = match r.num {
            Num::Stat(s) => c.stat_blurb(s),
            n => c.number_blurb(n.label()),
        };
        let blurb = wrap(blurb, width)
            .into_iter()
            .map(|l| Line::from(Span::styled(l, Style::default().fg(TEXT))))
            .collect();

        let mut feeds = Vec::new();
        if let Num::Stat(s) = r.num {
            let skills = c.skills.iter().filter(|k| k.stat == s).map(|k| {
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
        let mut parts = Vec::new();
        for (k, word) in KINDS.iter().enumerate() {
            let entries: Vec<Span<'static>> = r
                .sources
                .iter()
                .filter(|(f, _)| kind_of(f) == k)
                .map(|(from, v)| {
                    let text = match from {
                        Contrib::Gear(id) | Contrib::Chrome(id) | Contrib::Hardware(id) => {
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
                parts.extend(flow(
                    Span::styled(format!("{word} "), dim),
                    entries,
                    Span::styled(" · ", dim),
                    width,
                ));
            }
        }
        // An ability's part is its own line, named with what grants it.
        for (k, ability, by, v) in &r.boons {
            let by = by.as_ref().map_or(String::new(), |b| format!(" · {b}"));
            parts.extend(flow(
                Span::styled(format!("{} ", KINDS[*k]), dim),
                vec![Span::styled(
                    format!("{v:+} {ability}{by}"),
                    Style::default().fg(signed(*v)),
                )],
                Span::raw(""),
                width,
            ));
        }
        if let Some(note) = &r.note {
            parts.push(Line::from(Span::styled(
                cut(note, width),
                Style::default().fg(FAINTER),
            )));
        }
        Readout {
            head: Line::from(head),
            blurb,
            feeds,
            parts,
        }
    }

    /// The lines in at most `height` rows of `width`, counted as they wrap. Short of
    /// rows the blank line gives way first, then the blurb from its end (marked cut),
    /// then what a stat feeds, so where the number comes from stays to the last.
    fn fit(mut self, width: usize, height: usize) -> Vec<Line<'static>> {
        let rows = |lines: &[Line<'static>]| -> usize {
            lines.iter().map(|l| wrapped_rows(l, width)).sum()
        };
        let mut blank = !self.blurb.is_empty() && !(self.feeds.is_empty() && self.parts.is_empty());
        let mut cut_blurb = false;
        let lines = loop {
            let mut lines = vec![self.head.clone()];
            lines.extend(self.blurb.iter().cloned());
            if cut_blurb && let Some(last) = lines.last_mut().filter(|_| !self.blurb.is_empty()) {
                let text = format!("{last}…");
                *last = Line::from(Span::styled(cut(&text, width), Style::default().fg(TEXT)));
            }
            if blank {
                lines.push(Line::from(""));
            }
            lines.extend(self.feeds.iter().cloned());
            lines.extend(self.parts.iter().cloned());
            if rows(&lines) <= height {
                break lines;
            }
            if blank {
                blank = false;
            } else if self.blurb.pop().is_some() {
                cut_blurb = true;
            } else if self.feeds.pop().is_none() {
                break lines;
            }
        };
        let mut lines = lines;
        while rows(&lines) > height && lines.pop().is_some() {}
        lines
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
    /// The ledger's row.
    pub row: usize,
    /// The picker's pick, while it is open.
    pub picker: Option<usize>,
}

pub struct Sheet {
    pub state: SheetState,
    /// Tick of the last change of tile: drives the readout's typewriter.
    pub changed: u64,
}

impl Sheet {
    pub fn new(tick: u64) -> Sheet {
        Sheet {
            state: SheetState::default(),
            changed: tick,
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
        self.state.tile = self.tile();
        self.changed = tick;
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
                if let Num::Stat(s) = Num::ALL[tile] {
                    let result = perform(st, Action::BuyStat(s));
                    report(st, result);
                }
            }
            (SheetPane::Ledger, KeyCode::Left | KeyCode::Char('h')) => {
                self.state.pane = SheetPane::Left;
            }
            (
                SheetPane::Ledger,
                KeyCode::Right
                | KeyCode::Char('l')
                | KeyCode::Up
                | KeyCode::Char('k')
                | KeyCode::Down
                | KeyCode::Char('j')
                | KeyCode::Char(' ')
                | KeyCode::Enter,
            ) => {}
            _ => return false,
        }
        true
    }

    /// Draws the tab into `body`: the left pane's three blocks of tiles, the rule, the
    /// selected tile's readout typing out under it, and the rule between the panes.
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
                0 if st.character.stat_points > 0 => {
                    let n = st.character.stat_points;
                    vec![
                        (
                            n.to_string(),
                            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                        ),
                        (
                            format!(" point{}", if n == 1 { "" } else { "s" }),
                            Style::default().fg(DIM),
                        ),
                    ]
                }
                1 => vec![(
                    "from gear & chrome".to_string(),
                    Style::default().fg(FAINTER),
                )],
                _ => Vec::new(),
            };
            header(buf, (x0, right), y, title, &tail, focused);
            for (k, i) in span(b).enumerate() {
                let x = x0 + k as u16 * w;
                if y + 3 < body.bottom() && x + w <= right {
                    let sel = i == self.tile();
                    self.draw_tile(st, &readings[i], Rect::new(x, y + 1, w, 3), sel, buf);
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

        let rule_x = body.x + RULE_X;
        if rule_x < body.right() {
            for y in body.y..body.bottom() {
                buf[(rule_x, y)].set_char('║').set_fg(RULE);
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
                Num::Stat(s) => stat_colour(s),
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

    pub fn hints(&self, _st: &AppState) -> String {
        match self.state.pane {
            SheetPane::Left => {
                "←↑↓→ select · space buy a point · → over to the chrome · Q/E tabs · tab close"
            }
            SheetPane::Ledger => "← stats · Q/E tabs · tab close",
        }
        .to_string()
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
    let title_style = Style::default()
        .fg(if lit { CYAN } else { DIM })
        .add_modifier(Modifier::BOLD);
    put(buf, x0, y, title, title_style, right);
    let tail_w: u16 = tail
        .iter()
        .map(|(t, _)| t.chars().count() as u16)
        .sum::<u16>()
        + u16::from(!tail.is_empty());
    let start = x0 + title.chars().count() as u16 + 1;
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
    use crate::screens::create::stat_colour;
    use crate::screens::deck::{Deck, REVEAL_TICKS};
    use crate::screens::testutil::key;
    use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT};
    use citygen::{GenParams, Generator, SizePreset};
    use game::{Build, ItemId, Num, Site, Slot, Stat};
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
        (body.y + 16..body.bottom())
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
        assert_eq!(st.character.pieces(), 9);
    }

    #[test]
    fn left_pane_draws_its_tiles() {
        let mut st = state();
        st.character.stat_points = 2;
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
            "STATS",
            "2 points",
            "┌ BOD",
            "SECONDARY",
            "from gear & chrome",
            "┌ ARM",
            "RESISTANCES",
            "┌ EMP",
        ] {
            assert!(body.contains(want), "{want}: {body}");
        }
        // The mock's columns: the headers from x 2, their tails ending at x 44.
        assert_eq!(cells(buf, 2, 5, 5), "STATS");
        assert_eq!(cells(buf, 37, 5, 8), "2 points");
        assert_eq!(cells(buf, 2, 6, 16), "┌ BOD ─┐┌ REF ─┐");
        assert_eq!(cells(buf, 2, 10, 9), "SECONDARY");
        assert_eq!(cells(buf, 27, 10, 18), "from gear & chrome");
        assert_eq!(
            cells(buf, 2, 11, 42),
            "┌ HP ─┐┌ FOC ┐┌ ARM ┐┌ SGT ┐┌ CAR ┐┌ MOV ┐"
        );
        assert_eq!(cells(buf, 2, 15, 11), "RESISTANCES");
        assert_eq!(cells(buf, 2, 16, 14), "┌ KIN ┐┌ THM ┐");
        // The deck's +1 net in the NET tile's bottom edge, the jacket's +5 in HP's;
        // with no body SGT moved nothing, so its edge is bare.
        assert_eq!(cells(buf, 34, 8, 8), "└─ +1 ─┘");
        assert_eq!(buf[(37, 8)].fg, GOOD);
        assert_eq!(cells(buf, 2, 13, 7), "└─ +5 ┘");
        assert_eq!(cells(buf, 23, 13, 7), "└─────┘");
        // BODY, selected on the focused pane: cyan edge, the bar behind its value.
        assert_eq!(buf[(2, 6)].fg, CYAN);
        assert_eq!(buf[(5, 7)].bg, BAR_BG);
        assert_eq!(buf[(10, 6)].fg, RULE, "REF's edge, not selected");
        assert_eq!(
            buf[(12, 6)].fg,
            stat_colour(Stat::Reflex),
            "REF in its colour"
        );
        // Nothing of the left pane past body.x + 44; the rule stands at body.x + 45.
        for y in BODY.y..BODY.bottom() {
            assert_eq!(buf[(BODY.x + 44, y)].symbol(), " ", "y {y}");
            assert_eq!(buf[(BODY.x + 45, y)].symbol(), "║", "y {y}");
        }
    }

    #[test]
    fn keys_walk_the_tiles() {
        let mut st = state();
        st.character.stat_points = 3;
        let mut s = Sheet::new(0);
        assert_eq!(s.state.tile, 0, "BODY the first time");
        for _ in 0..3 {
            press(&mut s, &mut st, KeyCode::Right);
        }
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Right),
            tile_of(Num::Stat(Stat::Net))
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
            tile_of(Num::Stat(Stat::Net))
        );
        assert_eq!(s.state.pane, SheetPane::Left, "back to the tile it had");
        for _ in 0..3 {
            press(&mut s, &mut st, KeyCode::Char('h'));
        }
        assert_eq!(s.state.tile, tile_of(Num::Stat(Stat::Reflex)));
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
            tile_of(Num::Stat(Stat::Net))
        );
        assert_eq!(s.changed, 9);
        assert_eq!(press(&mut s, &mut st, KeyCode::Down), tile_of(Num::Carry));
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Char('k')),
            tile_of(Num::Stat(Stat::Net))
        );
        assert_eq!(
            press(&mut s, &mut st, KeyCode::Up),
            tile_of(Num::Stat(Stat::Net))
        );

        // Space on BODY buys a point; on ARM it does nothing.
        s.state.tile = 0;
        let cost =
            Build::from_character(&st.content, &st.character).stat_cost(&st.content, Stat::Body);
        let (body, points) = (st.character.stats.body, st.character.stat_points);
        press(&mut s, &mut st, KeyCode::Char(' '));
        assert_eq!(
            (st.character.stats.body, st.character.stat_points),
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
    fn readout_names_sources() {
        let mut st = state();
        st.character.stat_points = 3;
        let arm = ItemId::new("piston_arm");
        st.character.install(Site::LeftArm, arm, None);
        let mut s = Sheet::new(0);
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        let n =
            Build::from_character(&st.content, &st.character).stat_cost(&st.content, Stat::Body);
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
        assert_eq!(buf[(2, BODY.y + 16)].fg, stat_colour(Stat::Body));
        assert_eq!(buf[(BODY.x + 43, BODY.y + 16)].fg, AMBER);

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
        assert_eq!(cells(&buf, 23, 17, 7), "│ -18 │");
        assert_eq!(buf[(25, 17)].fg, MAGENTA);

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
            cells(&buf, 23, 12, 7).trim_matches(['│', ' ']),
            radius.to_string()
        );
        assert_eq!(cells(&buf, 23, 13, 7), "└─ +6 ┘");
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
        assert_eq!(cells(&buf, 23, 13, 7), "└─ +4 ┘");
        // No body: no sight, no edge, and the readout says why.
        st.player = None;
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, 23, 12, 7), "│  0  │");
        assert_eq!(cells(&buf, 23, 13, 7), "└─────┘");
        let text = readout(&buf, BODY);
        assert!(text.starts_with("SIGHT 0\n"), "{text}");
        assert!(text.contains("chrome +4 Nightglass optics"), "{text}");
        assert!(text.contains("no body: 0"), "{text}");
    }

    /// The readout's row that starts with `lead`, as drawn.
    fn row_of(buf: &Buffer, body: Rect, lead: &str) -> u16 {
        (body.y + 16..body.bottom())
            .find(|&y| cells(buf, body.x + 1, y, 43).starts_with(lead))
            .unwrap_or_else(|| panic!("no row starts with {lead}"))
    }

    #[test]
    fn the_edge_is_what_the_value_moved_and_the_readout_says_why() {
        // BODY 8, a Wrecker arm and a Piston arm: three more, but BODY stops at 10.
        let mut st = state();
        st.character.stats.body = 8;
        st.character
            .install(Site::LeftArm, ItemId::new("wrecker_arm"), None);
        st.character
            .install(Site::RightArm, ItemId::new("piston_arm"), None);
        let s = Sheet::new(0);
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, 2, 7, 8), "│  10  │");
        assert_eq!(cells(&buf, 2, 8, 8), "└─ +2 ─┘");
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
        assert_eq!(cells(&buf, mov, 12, 7), "│  4  │");
        assert_eq!(
            cells(&buf, mov, 13, 7),
            "└─────┘",
            "no edge: it moved nothing"
        );
        assert_eq!(buf[(mov + 3, 12)].fg, TEXT);
        let text = readout(&buf, BODY);
        assert!(text.starts_with("MOVE 4\n"), "{text}");
        assert!(text.contains("chrome +1 Redline spine"), "{text}");
        assert!(text.contains("in a construct: 4"), "{text}");
        // On foot the spine counts, and nothing needs saying.
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Street);
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, mov, 13, 7), "└─ +1 ┘");
        let text = readout(&buf, BODY);
        assert!(text.starts_with("MOVE 4 · 5 with chrome"), "{text}");
        assert!(!text.contains("in a construct:"), "{text}");
    }

    #[test]
    fn line_one_fits_beside_the_cost_and_no_source_is_lost() {
        // COOL from the mirrorshades and the Hushcan on the Ratchet, points unspent.
        let mut st = state();
        st.character.stat_points = 3;
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
        s.state.tile = tile_of(Num::Stat(Stat::Cool));
        settle(&s, &mut st);
        let buf = drawn(&s, &st, BODY);
        let text = readout(&buf, BODY);
        let n =
            Build::from_character(&st.content, &st.character).stat_cost(&st.content, Stat::Cool);
        let next = format!("next {n} pt{}", if n == 1 { "" } else { "s" });
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("COOL 5 · 7 in all"), "{text}");
        assert!(
            first.ends_with(&next),
            "one row, the cost at the right: {text}"
        );
        assert_eq!(buf[(BODY.x + 43, BODY.y + 16)].fg, AMBER);
        for want in ["feeds", "gear +1 Mirrorshades", "hardware +1 Hushcan"] {
            assert!(text.contains(want), "{want}: {text}");
        }
        // The blurb gave way to make room, and says so.
        assert!(text.lines().nth(1).unwrap().ends_with('…'), "{text}");
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
        assert_eq!(buf[(2, 6)].fg, DIM, "BODY keeps its place, dim");
        assert_ne!(buf[(5, 7)].bg, BAR_BG);
        assert_eq!(buf[(2, 5)].fg, DIM, "the headers go dim");
        assert!(s.hints(&st).contains("← stats"));
        s.state.pane = SheetPane::Left;
        assert_eq!(
            s.hints(&st),
            "←↑↓→ select · space buy a point · → over to the chrome · Q/E tabs · tab close"
        );
    }

    #[test]
    fn nothing_spills_past_the_left_pane() {
        let mut st = walking();
        nine_pieces(&mut st);
        st.character.stat_points = 12;
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
                    assert_eq!(buf[(body.x, body.y + 4)].symbol(), " ");
                    // Every line of where the number comes from is drawn.
                    let text = readout(&buf, body);
                    let r = Reading::of(&st, Num::ALL[tile]);
                    for part in Readout::of(&st, &r, 43).parts {
                        let part = part.to_string();
                        assert!(
                            text.contains(part.trim_end()),
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
        assert_eq!(cells(&wide, 2, WIDE.y, 5), "STATS");
        assert_eq!(cells(&wide, 2, WIDE.y + 1, 8), "┌ BOD ─┐");
        assert!(readout(&wide, WIDE).starts_with("BODY 4"));
        assert_eq!(wide[(WIDE.x + 45, WIDE.bottom() - 1)].symbol(), "║");
    }
}
