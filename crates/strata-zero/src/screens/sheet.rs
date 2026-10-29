//! STATS, the character sheet: on the left the stats you buy, the secondary numbers
//! gear and chrome move and the resistances, as rack tiles with a readout of the one
//! selected; on the right the ledger of the chrome in you.

use crate::actions::{perform, report};
use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use crate::screens::create::{stat_colour, wrap};
use crate::screens::deck::{REVEAL_TICKS, cut, lit};
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT, put, put_paragraph, reveal, solid,
};
use citygen::{Place, StratumKind};
use crossterm::event::KeyCode;
use game::{Action, Build, Contrib, Num};
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

/// The radius you see by where you stand, as the map's field of view reads it; no
/// body, no sight.
fn sight(st: &AppState) -> i32 {
    let Some(p) = st.player.as_ref() else {
        return 0;
    };
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
    (p.radius_at(daylight) + st.sight_bonus()).max(1)
}

/// One number as the sheet reads it: with everything, what it is before gear, chrome
/// and talents (`None` for one that is nothing but theirs), and where the rest comes
/// from.
struct Reading {
    num: Num,
    value: i32,
    base: Option<i32>,
    sources: Vec<(Contrib, i32)>,
}

impl Reading {
    fn of(st: &AppState, num: Num) -> Reading {
        let (c, ch) = (&st.content, &st.character);
        let sources = ch.sources(c, num);
        let added: i32 = sources.iter().map(|(_, v)| v).sum();
        let value = match num {
            Num::Stat(s) => ch.stat(c, s) as i32,
            Num::Hp => ch.hp_max(c) as i32,
            Num::Focus => ch.focus_max(c) as i32,
            Num::Armor => ch.armor(c),
            Num::Sight => sight(st),
            Num::Carry => ch.carry_cap(c).round() as i32,
            Num::Move => {
                let place = st
                    .player
                    .as_ref()
                    .map_or(Place::Stratum(StratumKind::Street), |p| p.place);
                crate::net::you_move(st, place) as i32
            }
            Num::Resist(h) => ch.resist(c, h),
        };
        let base = match num {
            Num::Stat(s) => Some(ch.stats.get(s) as i32),
            Num::Hp => Some(ch.hp_base as i32),
            Num::Focus => Some(ch.focus_base as i32),
            Num::Move => Some(crate::net::BASE_MOVE as i32),
            Num::Sight if st.player.is_none() => None,
            Num::Sight | Num::Carry => Some(value - added),
            Num::Armor | Num::Resist(_) => None,
        };
        Reading {
            num,
            value,
            base,
            sources,
        }
    }

    /// What gear, chrome and talents add.
    fn added(&self) -> i32 {
        self.sources.iter().map(|(_, v)| v).sum()
    }

    /// The value's colour: a stat plain, a secondary number by its bonus, a
    /// resistance by its sign.
    fn colour(&self) -> Color {
        match self.num {
            Num::Stat(_) => TEXT,
            Num::Resist(_) => signed(self.value),
            _ => signed(self.added()),
        }
    }

    /// The tile's bottom edge: what gear, chrome and talents add. A resistance is
    /// nothing but that, so it shows only its value.
    fn modifier(&self) -> Option<(String, Color)> {
        let added = self.added();
        (added != 0 && !matches!(self.num, Num::Resist(_)))
            .then(|| (format!("{added:+}"), signed(added)))
    }
}

/// The kinds of source, in the order the readout lists them, each with its word.
const KINDS: [&str; 4] = ["gear", "hardware", "chrome", "talents"];

fn kind_of(from: &Contrib) -> usize {
    match from {
        Contrib::Gear(_) => 0,
        Contrib::Hardware(_) => 1,
        Contrib::Chrome(_) | Contrib::Pieces(_) => 2,
        Contrib::Talents => 3,
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

/// The readout of a number, `width` columns wide: its name and value, its blurb,
/// what a stat feeds, and where the rest comes from, one line for each kind.
fn readout(st: &AppState, r: &Reading, width: usize) -> Vec<Line<'static>> {
    let (c, ch) = (&st.content, &st.character);
    let bold = |colour: Color| Style::default().fg(colour).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(DIM);
    let name = match r.num {
        Num::Stat(s) => stat_colour(s),
        _ => r.colour(),
    };
    let mut head = vec![Span::styled(format!("{} ", r.num.name()), bold(name))];
    match r.base {
        Some(base) if base != r.value => {
            let mut kinds: Vec<usize> = r.sources.iter().map(|(f, _)| kind_of(f)).collect();
            kinds.sort_unstable();
            kinds.dedup();
            let words: Vec<&str> = kinds.iter().map(|&k| KINDS[k]).collect();
            let with = match words.split_last() {
                Some((last, [])) => last.to_string(),
                Some((last, rest)) => format!("{} & {last}", rest.join(", ")),
                None => "everything".to_string(),
            };
            head.push(Span::styled(base.to_string(), bold(TEXT)));
            head.push(Span::styled(format!(" · {} with {with}", r.value), dim));
        }
        _ => head.push(Span::styled(r.value.to_string(), bold(r.colour()))),
    }
    // What the next point costs, at the right, while there are points and room.
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
    let mut out = vec![Line::from(head)];

    let blurb = match r.num {
        Num::Stat(s) => c.stat_blurb(s),
        n => c.number_blurb(n.label()),
    };
    out.extend(
        wrap(blurb, width)
            .into_iter()
            .map(|l| Line::from(Span::styled(l, Style::default().fg(TEXT)))),
    );
    out.push(Line::from(""));

    if let Num::Stat(s) = r.num {
        let skills = c.skills.iter().filter(|k| k.stat == s).map(|k| {
            let mark = if ch.has_skill(&k.id) { "◆" } else { "◇" };
            Span::styled(format!("{mark} {}", k.name), Style::default().fg(TEXT))
        });
        out.extend(flow(
            Span::styled("feeds ", dim),
            skills.collect(),
            Span::raw(" "),
            width,
        ));
    }

    for (k, word) in KINDS.iter().enumerate() {
        let entries: Vec<Span<'static>> = r
            .sources
            .iter()
            .filter(|(f, _)| kind_of(f) == k)
            .map(|(from, v)| {
                let item = |id: &game::ItemId| {
                    c.item(id)
                        .map_or_else(|| id.to_string(), |i| i.name.clone())
                };
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
            out.extend(flow(
                Span::styled(format!("{word} "), dim),
                entries,
                Span::styled(" · ", dim),
                width,
            ));
        }
    }
    while out.last().is_some_and(|l| l.width() == 0) {
        out.pop();
    }
    out
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
            let mut lines = readout(st, &readings[self.tile()], area.width as usize);
            // Short of rows, the blank ones give way first.
            while lines.len() > area.height as usize
                && let Some(blank) = lines.iter().rposition(|l| l.width() == 0)
            {
                lines.remove(blank);
            }
            lines.truncate(area.height as usize);
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
    use crate::ui::chrome::{BAR_BG, CYAN, DIM, MAGENTA, RULE};
    use citygen::{GenParams, Generator, SizePreset};
    use game::{Build, ItemId, Num, Site, Stat};
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
        // The deck's +1 net in the NET tile's bottom edge, the optics' +4 in SGT's.
        assert_eq!(cells(buf, 34, 8, 8), "└─ +1 ─┘");
        assert_eq!(buf[(37, 8)].fg, GOOD);
        assert_eq!(cells(buf, 23, 13, 7), "└─ +4 ┘");
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
        let radius = (p.radius_at(map.daylight()) + st.sight_bonus()).max(1);
        assert!(radius > 4, "the street's radius and the optics: {radius}");
        let buf = drawn(&s, &st, BODY);
        assert_eq!(
            cells(&buf, 23, 12, 7).trim_matches(['│', ' ']),
            radius.to_string()
        );
        let text = readout(&buf, BODY);
        assert!(
            text.starts_with(&format!("SIGHT {} · {radius} with chrome", radius - 4)),
            "{text}"
        );
        assert!(text.contains("chrome +4 Nightglass optics"), "{text}");
        // No body: no sight.
        st.player = None;
        let buf = drawn(&s, &st, BODY);
        assert_eq!(cells(&buf, 23, 12, 7), "│  0  │");
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
