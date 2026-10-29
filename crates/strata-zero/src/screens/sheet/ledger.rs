//! The sheet's right pane: the ledger of the body's sites with a chrome limb's sockets
//! folded under it, the card of the row the cursor is on, the picker of what in the pack
//! fits a site or socket (swapping out what it holds), and the LOAD gauge over them.

use super::{RULE_X, Sheet, SheetPane, header};
use crate::actions::{JACKED_IN, jacked_in, perform, report};
use crate::app::AppState;
use crate::screens::create::wrap;
use crate::screens::deck::{REVEAL_TICKS, cut, lit};
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT, put, reveal, solid};
use crate::ui::hud::{AMBER, GREEN};
use crate::ui::implant_art;
use crossterm::event::KeyCode;
use game::{
    Action, Content, CopyId, EMP_PER_PIECE, Fits, Host, Item, ItemId, Perk, Site, SocketKind,
    stack_load,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// The right pane's first column, from the body's left edge: two past the rule.
const RIGHT_X: u16 = RULE_X + 2;
/// The ledger's width; on a body 120 columns wide and more, where each row names its
/// site, wider.
const LEDGER_W: u16 = 18;
const LEDGER_WIDE: u16 = 27;
/// A 120-column screen's body.
const WIDE_BODY: u16 = 118;
/// The wide ledger's site labels, before what each holds.
const LABEL_W: u16 = 8;
/// The LOAD gauge's segments.
const SEGMENTS: u32 = 12;
/// Rows from the body's top to the ledger and the card: the CHROME header, then a
/// blank row, as under the left pane's headers.
const BELOW_HEADER: u16 = 2;
/// The card's socket column: `■ hand    Arc hand`.
const SOCKET_W: usize = 10;

/// A row of the ledger: a group's header, a site, or a socket of the piece a site holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerRow {
    Group(&'static str),
    Site(Site),
    Socket(Site, usize),
}

impl Default for LedgerRow {
    fn default() -> LedgerRow {
        LedgerRow::Site(Site::ALL[0])
    }
}

impl LedgerRow {
    /// Where on the body the row is: a site (`None`), or a socket of what a site holds.
    /// A group's header is nowhere.
    fn at(self) -> Option<(Site, Option<usize>)> {
        match self {
            LedgerRow::Group(_) => None,
            LedgerRow::Site(s) => Some((s, None)),
            LedgerRow::Socket(s, n) => Some((s, Some(n))),
        }
    }
}

/// A piece in the pack that fits where the cursor is: its copy when it is held per copy,
/// and what installing it adds to the load, with what is fitted in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub item: ItemId,
    pub copy: Option<CopyId>,
    pub load: u32,
}

/// The ledger's rows: every site in `Site::ALL` order under its group's header, and
/// under a piece with sockets one row for each.
pub fn ledger_rows(st: &AppState) -> Vec<LedgerRow> {
    let (c, ch) = (&st.content, &st.character);
    let mut rows = Vec::new();
    let mut group = None;
    for site in Site::ALL {
        if group != Some(site.group()) {
            group = Some(site.group());
            rows.push(LedgerRow::Group(site.group()));
        }
        rows.push(LedgerRow::Site(site));
        let sockets = ch.chrome_at(site).and_then(|id| c.item(id));
        let n = sockets.map_or(0, |i| i.sockets.len());
        rows.extend((0..n).map(|k| LedgerRow::Socket(site, k)));
    }
    rows
}

/// What in the pack fits `at`, a site or socket, empty or not, in pack order: a copy
/// each, a stack without one once. A socket takes copies only, as `Fit` moves them.
/// Another of the piece a site holds without a copy is that same piece: no swap.
pub fn fitting(st: &AppState, at: (Site, Option<usize>)) -> Vec<Pick> {
    let (c, ch) = (&st.content, &st.character);
    let same = match at {
        (site, None) if ch.chrome_copy(site).is_none() => ch.chrome_at(site),
        _ => None,
    };
    let mut out: Vec<Pick> = Vec::new();
    for s in &ch.inventory {
        if s.qty == 0 || !ch.fits_at(c, &s.item, at) || (at.1.is_some() && s.copy.is_none()) {
            continue;
        }
        if s.copy.is_none() && same == Some(&s.item) {
            continue;
        }
        if s.copy.is_none() && out.iter().any(|p| p.copy.is_none() && p.item == s.item) {
            continue;
        }
        out.push(Pick {
            item: s.item.clone(),
            copy: s.copy_id(),
            load: stack_load(c, s),
        });
    }
    out
}

/// The load once `pick` is in at `at`: what the place holds leaves first, with what is
/// fitted in it.
fn load_after(st: &AppState, at: (Site, Option<usize>), pick: &Pick) -> u32 {
    let (c, ch) = (&st.content, &st.character);
    ch.chrome_load(c).saturating_sub(ch.load_at(c, at)) + pick.load
}

/// The pieces at `at` that a pick swaps out: what the place holds, with what is fitted
/// in it.
fn held_pieces(st: &AppState, (site, socket): (Site, Option<usize>)) -> usize {
    st.character
        .installed()
        .iter()
        .filter(|&&(s, n, _)| s == site && (socket.is_none() || n == socket))
        .count()
}

/// The pieces a pick installs: itself and what is fitted in it.
fn pick_pieces(st: &AppState, pick: &Pick) -> usize {
    let stack = st.character.inventory.iter().find(|s| match pick.copy {
        Some(k) => s.copy_id() == Some(k),
        None => s.copy.is_none() && s.item == pick.item,
    });
    let fitted = stack
        .and_then(|s| s.copy.as_ref())
        .map_or(0, |k| k.fitted.iter().flatten().count());
    1 + fitted
}

/// What a site or socket holds.
fn held(st: &AppState, (site, socket): (Site, Option<usize>)) -> Option<&ItemId> {
    let ch = &st.character;
    match socket {
        None => ch.chrome_at(site),
        Some(n) => ch
            .chrome_copy(site)?
            .fitted
            .get(n)?
            .as_ref()
            .map(|s| &s.item),
    }
}

/// The kind of a socket of what a site holds.
fn socket_kind(st: &AppState, site: Site, n: usize) -> Option<SocketKind> {
    let item = st
        .character
        .chrome_at(site)
        .and_then(|id| st.content.item(id))?;
    item.sockets.get(n).copied()
}

/// A socket's own word, from its kind.
fn socket_word(st: &AppState, site: Site, n: usize) -> &'static str {
    socket_kind(st, site, n).map_or("socket", SocketKind::label)
}

/// The name of what a site holds.
fn host_name(st: &AppState, site: Site) -> String {
    let id = st.character.chrome_at(site);
    id.and_then(|id| st.content.item(id)).map_or_else(
        || id.map_or_else(String::new, |id| id.to_string()),
        |i| i.name.clone(),
    )
}

/// The row the cursor stands on: `want` while it is still a row, its site when a
/// socket went with its limb, else the first site.
fn resolve(rows: &[LedgerRow], want: LedgerRow) -> usize {
    let find = |r: LedgerRow| rows.iter().position(|&x| x == r);
    want.at()
        .and_then(|_| find(want))
        .or_else(|| match want {
            LedgerRow::Socket(site, _) => find(LedgerRow::Site(site)),
            _ => None,
        })
        .or_else(|| rows.iter().position(|r| r.at().is_some()))
        .unwrap_or(0)
}

/// The right pane as it stands.
struct View {
    rows: Vec<LedgerRow>,
    /// The cursor's row, an index into `rows`.
    row: usize,
    at: (Site, Option<usize>),
    /// What the cursor's site or socket holds.
    held: Option<ItemId>,
    /// What in the pack fits there; a pick swaps out what it holds.
    picks: Vec<Pick>,
    /// The pick, while the picker is open.
    pick: Option<usize>,
}

impl Sheet {
    fn view(&self, st: &AppState) -> View {
        let rows = ledger_rows(st);
        let row = resolve(&rows, self.state.row);
        let at = rows[row].at().unwrap_or((Site::ALL[0], None));
        let held = held(st, at).cloned();
        let picks = fitting(st, at);
        let pick = self
            .state
            .picker
            .filter(|_| self.state.pane == SheetPane::Ledger && !picks.is_empty())
            .map(|p| p.min(picks.len() - 1));
        View {
            rows,
            row,
            at,
            held,
            picks,
            pick,
        }
    }

    /// Puts the cursor back on a row that exists (a socket's limb gone, its site) and
    /// closes a picker that has nothing left to pick.
    pub fn reselect(&mut self, st: &AppState) {
        let v = self.view(st);
        let row = v.rows[v.row];
        if row != self.state.row {
            self.state.row = row;
            self.card = st.tick;
        }
        if self.state.picker.is_some() && v.pick != self.state.picker {
            if v.pick.is_none() {
                self.card = st.tick;
            }
            self.state.picker = v.pick;
        }
    }

    /// The ledger's keys, and the picker's while it is open; true when it was the
    /// sheet's.
    pub(super) fn ledger_key(&mut self, st: &mut AppState, code: KeyCode) -> bool {
        let tick = st.tick;
        let v = self.view(st);
        if let Some(pick) = v.pick {
            match code {
                // The picker works inside the place's card: moving the pick changes
                // lines in place and never types the card out again. Closing it does
                // only on a held place, whose own card comes back (esc, below).
                KeyCode::Up | KeyCode::Char('k') => {
                    self.state.picker = Some(pick.saturating_sub(1))
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.state.picker = Some((pick + 1).min(v.picks.len() - 1))
                }
                KeyCode::Char(' ') | KeyCode::Enter => self.install(st, &v, pick),
                // Back on a piece, its own card types out again.
                KeyCode::Esc => {
                    self.state.picker = None;
                    if v.held.is_some() {
                        self.card = tick;
                    }
                }
                // The picker keeps the cursor until it closes.
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {}
                _ => return false,
            }
            return true;
        }
        match code {
            KeyCode::Left | KeyCode::Char('h') => self.state.pane = SheetPane::Left,
            KeyCode::Up | KeyCode::Char('k') => {
                let up = v.rows[..v.row].iter().rposition(|r| r.at().is_some());
                self.walk(&v, up, tick);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let down = v.rows[v.row + 1..].iter().position(|r| r.at().is_some());
                self.walk(&v, down.map(|i| v.row + 1 + i), tick);
            }
            KeyCode::Right | KeyCode::Char('l') => {}
            KeyCode::Char(' ') | KeyCode::Enter => self.act(st, &v),
            KeyCode::Char('x') => self.pull(st, &v),
            _ => return false,
        }
        true
    }

    fn walk(&mut self, v: &View, to: Option<usize>, tick: u64) {
        if let Some(i) = to {
            self.state.row = v.rows[i];
            self.card = tick;
        }
    }

    /// Space on a row opens what in the pack fits there, empty or not, or says nothing
    /// does. On a piece the FITS card takes the place of its own and types out.
    fn act(&mut self, st: &mut AppState, v: &View) {
        if jacked_in(st) {
            st.warn(JACKED_IN);
        } else if v.picks.is_empty() {
            st.warn("nothing in your bag fits");
        } else {
            self.state.picker = Some(0);
            if v.held.is_some() {
                self.card = st.tick;
            }
        }
    }

    /// `x` on a row: chrome comes out to the pack, a limb with what is in it; an empty
    /// place has nothing to pull.
    fn pull(&mut self, st: &mut AppState, v: &View) {
        if v.held.is_none() {
            return;
        }
        let (site, socket) = v.at;
        let action = match socket {
            None => Action::Pull(site),
            Some(socket) => Action::Unfit {
                host: Host::Site(site),
                socket,
            },
        };
        let result = perform(st, action);
        if result.is_ok() {
            self.card = st.tick;
        }
        report(st, result);
    }

    /// Installs the pick, or fits it in its socket, what was there going to the pack;
    /// it closes the picker, and a refusal lands in the strip with the picker still open.
    fn install(&mut self, st: &mut AppState, v: &View, pick: usize) {
        let p = &v.picks[pick];
        let (site, socket) = v.at;
        let action = match (socket, p.copy) {
            (None, Some(copy)) => Action::InstallCopy { copy, site },
            (None, None) => Action::Install {
                item: p.item.clone(),
                site,
            },
            (Some(socket), Some(piece)) => Action::Fit {
                host: Host::Site(site),
                socket,
                piece,
            },
            // `fitting` offers a socket copies only.
            (Some(_), None) => return,
        };
        let result = perform(st, action);
        if result.is_ok() {
            self.state.picker = None;
            self.card = st.tick;
        }
        report(st, result);
    }

    /// The ledger's hint row: what space does, and on chrome what `x` does, dimmer
    /// while the lock is on.
    pub(super) fn ledger_hints(&self, st: &AppState) -> Line<'static> {
        let v = self.view(st);
        let dim = Style::default().fg(DIM);
        if v.pick.is_some() {
            // On a held place the pick swaps with what is there.
            let space = match v.held {
                Some(_) => "space swap",
                None => "space install",
            };
            return Line::from(Span::styled(
                format!("↑↓ choose · {space} · esc back · Q/E tabs · tab close"),
                dim,
            ));
        }
        let locked = if jacked_in(st) {
            Style::default().fg(FAINTER)
        } else {
            dim
        };
        let mut spans = vec![
            Span::styled("↑↓ select · ", dim),
            Span::styled("space what fits", locked),
        ];
        if v.held.is_some() {
            spans.push(Span::styled(" · ", dim));
            spans.push(Span::styled("x pull it out", locked));
        }
        spans.push(Span::styled(" · ← attributes · Q/E tabs · tab close", dim));
        Line::from(spans)
    }

    /// Draws the right pane into `body`'s columns past the rule: the CHROME header with
    /// its LOAD gauge, the ledger, its rule and the card.
    pub(super) fn draw_right(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        let x0 = body.x + RIGHT_X;
        let x1 = body.right().saturating_sub(1);
        if x0 + 2 >= x1 || body.height < 2 {
            return;
        }
        // Solid, as the left pane is: the map's ghost stays out of the gaps.
        let pane = body.x + RULE_X + 1;
        solid(
            buf,
            Rect::new(pane, body.y, body.right() - pane, body.height),
        );
        let v = self.view(st);
        let focused = self.state.pane == SheetPane::Ledger;
        let wide = body.width >= WIDE_BODY;
        let rule = (x0 + if wide { LEDGER_WIDE } else { LEDGER_W }).min(x1);
        let after = v.pick.map(|p| load_after(st, v.at, &v.picks[p]));
        chrome_header(st, (x0, x1), body.y, focused, after, buf);
        let (top, height) = (
            body.y + BELOW_HEADER,
            body.height.saturating_sub(BELOW_HEADER),
        );
        let rows = Rect::new(x0, top, rule - x0, height);
        draw_ledger(st, &v, rows, focused && v.pick.is_none(), wide, buf);
        for y in body.y + 1..body.bottom() {
            buf[(rule, y)].set_char('│').set_fg(RULE);
        }
        let cx = rule + 2;
        if cx < x1 {
            let card = Rect::new(cx, top, x1 - cx, height);
            self.draw_card(st, &v, card, focused, buf);
        }
    }

    /// The card of the cursor's row, typing out: a piece's drawing and what it is, or
    /// what in the pack fits an empty place, or a held one while the picker is open.
    fn draw_card(&self, st: &AppState, v: &View, area: Rect, focused: bool, buf: &mut Buffer) {
        let p = st.tick.saturating_sub(self.card) as f32 / REVEAL_TICKS as f32;
        let cursor_on = (st.tick / 4).is_multiple_of(2);
        let w = area.width as usize;
        let mut text = area;
        let card = match (&v.held, v.pick) {
            (Some(id), None) => {
                // The drawing scans in over the first half of the reveal.
                if let Some(art) = st.content.implant_art(id) {
                    let scan = ((2.0 * p).min(1.0) * f32::from(implant_art::ROWS)).ceil() as u16;
                    implant_art::draw(buf, area.x, area.y, area, art, scan);
                    let drop = (implant_art::ROWS + 1).min(area.height);
                    text.y += drop;
                    text.height -= drop;
                }
                piece_card(st, v.at, id, w)
            }
            _ => fit_card(st, v, focused, w),
        };
        let card = card.fit(text.height as usize, w);
        draw_lines(buf, text, &reveal(&card.lines, p, cursor_on), &card.bars);
    }
}

/// `CHROME ──── LOAD ▰▰▰▱ 20/24`: the load's share of the budget in twelve segments,
/// rounded up. With a pick, the load `after` it goes in, what leaves counted: what it
/// adds in amber and the total with it; a lighter one, the lower share and total.
fn chrome_header(
    st: &AppState,
    (x0, x1): (u16, u16),
    y: u16,
    focused: bool,
    after: Option<u32>,
    buf: &mut Buffer,
) {
    let ch = &st.character;
    let (load, max) = (ch.chrome_load(&st.content), ch.load_max());
    let share = |n: u32| match max {
        0 => SEGMENTS,
        m => (n * SEGMENTS).div_ceil(m).min(SEGMENTS),
    };
    let total = after.unwrap_or(load);
    let more = total > load;
    let (have, after) = (share(load.min(total)), share(total));
    let fg = |c: Color| Style::default().fg(c);
    let tail = [
        ("LOAD ".to_string(), fg(DIM)),
        (
            "▰".repeat(have as usize),
            fg(if focused { CYAN } else { DIM }),
        ),
        ("▰".repeat((after - have) as usize), fg(AMBER)),
        ("▱".repeat((SEGMENTS - after) as usize), fg(FAINTER)),
        (
            format!(" {total}"),
            fg(if more { AMBER } else { TEXT }).add_modifier(Modifier::BOLD),
        ),
        (format!("/{max}"), fg(DIM)),
    ];
    header(buf, (x0, x1), y, "CHROME", &tail, focused);
}

/// The ledger's rows into `area`, scrolled to keep the cursor in sight: groups' headers
/// faint, a site's piece or its label lower case, a socket's piece or its kind under
/// the piece. The cursor's row has the mark, and the bar while the ledger has focus.
fn draw_ledger(st: &AppState, v: &View, area: Rect, focused: bool, wide: bool, buf: &mut Buffer) {
    let right = area.right();
    let height = area.height as usize;
    let first = match v.rows.len().checked_sub(height) {
        Some(over) if over > 0 => v.row.saturating_sub(height / 2).min(over),
        _ => 0,
    };
    let name_x = area.x + 2 + if wide { LABEL_W } else { 0 };
    let fg = |c: Color| Style::default().fg(c);
    for (k, (i, &row)) in v
        .rows
        .iter()
        .enumerate()
        .skip(first)
        .take(height)
        .enumerate()
    {
        let y = area.y + k as u16;
        let Some(at) = row.at() else {
            if let LedgerRow::Group(g) = row {
                put(
                    buf,
                    area.x + 1,
                    y,
                    &format!("── {g} ──"),
                    fg(FAINTER),
                    right,
                );
            }
            continue;
        };
        let on = i == v.row;
        let bar = on && focused;
        let bg = |s: Style| if bar { s.bg(BAR_BG) } else { s };
        if bar {
            for x in area.x..right {
                buf[(x, y)].set_bg(BAR_BG);
            }
        }
        if on {
            let mark = fg(if focused { lit(st.tick) } else { DIM });
            put(
                buf,
                area.x,
                y,
                "▸",
                bg(mark).add_modifier(Modifier::BOLD),
                right,
            );
        }
        let (x, empty) = match row {
            LedgerRow::Socket(site, n) => {
                let last = !matches!(v.rows.get(i + 1), Some(LedgerRow::Socket(..)));
                let branch = if last { "└ " } else { "├ " };
                put(buf, name_x + 1, y, branch, bg(fg(RULE)), right);
                (name_x + 3, format!("□ {}", socket_word(st, site, n)))
            }
            LedgerRow::Site(site) if wide => {
                put(buf, area.x + 2, y, site.label(), bg(fg(FAINTER)), right);
                (name_x, "· meat".to_string())
            }
            LedgerRow::Site(site) => (name_x, format!("· {}", site.lower())),
            LedgerRow::Group(_) => continue,
        };
        let room = right.saturating_sub(x) as usize;
        let (text, style) = match held(st, at).and_then(|id| st.content.item(id)) {
            Some(item) => {
                let colour = match (on, focused) {
                    (true, true) => CYAN,
                    (true, false) => TEXT,
                    _ if at.1.is_none() => TEXT,
                    _ => DIM,
                };
                let style = fg(colour);
                (
                    item.name.clone(),
                    if on {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        style
                    },
                )
            }
            None if bar => (empty, fg(lit(st.tick)).add_modifier(Modifier::BOLD)),
            None => (empty, fg(FAINTER)),
        };
        put(buf, x, y, &cut(&text, room), bg(style), right);
    }
}

/// A card's lines, a row each, which of them sit on the bar, and where its description
/// starts, if it has one.
#[derive(Default)]
struct CardText {
    lines: Vec<Line<'static>>,
    bars: Vec<bool>,
    desc: Option<usize>,
}

impl CardText {
    fn push(&mut self, line: Line<'static>) {
        self.lines.push(line);
        self.bars.push(false);
    }

    fn extend(&mut self, lines: Vec<Line<'static>>) {
        for line in lines {
            self.push(line);
        }
    }

    fn blank(&mut self) {
        self.push(Line::default());
    }

    /// The description, wrapped, after a blank row.
    fn description(&mut self, text: &str, w: usize) {
        self.blank();
        self.desc = Some(self.lines.len());
        for line in wrap(text, w) {
            self.push(Line::from(Span::styled(line, Style::default().fg(TEXT))));
        }
    }

    /// Fits `rows`: blank rows give way first, top down, then the description from its
    /// end, its last line kept marked cut; then the bottom.
    fn fit(mut self, rows: usize, w: usize) -> CardText {
        let mut cut_desc = false;
        while self.lines.len() > rows {
            if let Some(i) = self.lines.iter().position(|l| l.width() == 0) {
                self.lines.remove(i);
                self.bars.remove(i);
                self.desc = self.desc.map(|d| if i < d { d - 1 } else { d });
            } else if self.desc.is_some_and(|d| self.lines.len() > d) {
                self.lines.pop();
                self.bars.pop();
                cut_desc = true;
            } else {
                self.lines.truncate(rows);
                self.bars.truncate(rows);
                cut_desc = false;
            }
        }
        if cut_desc
            && self.desc.is_some_and(|d| self.lines.len() > d)
            && let Some(last) = self.lines.last_mut()
        {
            let text = format!("{last}…");
            *last = Line::from(Span::styled(cut(&text, w), Style::default().fg(TEXT)));
        }
        self
    }
}

/// An installed piece's card, under its drawing: the name, where it sits and what it
/// costs, what it does; a limb's sockets and what is in each; the description.
fn piece_card(
    st: &AppState,
    (site, socket): (Site, Option<usize>),
    id: &ItemId,
    w: usize,
) -> CardText {
    let c = &st.content;
    let mut card = CardText::default();
    let fg = |c: Color| Style::default().fg(c);
    let Some(item) = c.item(id) else {
        card.push(Line::from(Span::styled(id.to_string(), fg(TEXT))));
        return card;
    };
    card.push(Line::from(Span::styled(
        cut(&item.name.to_uppercase(), w),
        fg(CYAN).add_modifier(Modifier::BOLD),
    )));
    let place = match socket {
        None => site.label().to_string(),
        Some(n) => format!(
            "{} · on the {}",
            socket_word(st, site, n),
            host_name(st, site)
        ),
    };
    let load = item.implant.map_or(0, |i| i.load);
    card.extend(wrap_spans(
        vec![
            Span::styled(format!("{place} · load {load} · "), fg(DIM)),
            Span::styled(format!("EMP {}", signed(-EMP_PER_PIECE)), fg(MAGENTA)),
        ],
        w,
    ));
    card.extend(wrap_spans(effects(c, item, false), w));
    if socket.is_none() && !item.sockets.is_empty() {
        card.blank();
        let copy = st.character.chrome_copy(site);
        for (n, &kind) in item.sockets.iter().enumerate() {
            let fitted = copy
                .and_then(|k| k.fitted.get(n)?.as_ref())
                .and_then(|s| c.item(&s.item));
            card.extend(socket_lines(c, kind, fitted, w));
        }
    }
    card.description(&item.desc, w);
    card
}

/// A limb's socket on its card: `■ hand    Arc hand` with the piece's effects under
/// it, or `□ forearm empty` with what in content fits it, faint and cut.
fn socket_lines(
    c: &Content,
    kind: SocketKind,
    fitted: Option<&Item>,
    w: usize,
) -> Vec<Line<'static>> {
    let fg = |c: Color| Style::default().fg(c);
    let label = |mark: &str| format!("{mark} {:<width$} ", kind.label(), width = SOCKET_W - 3);
    let indent = label("■").chars().count();
    let room = w.saturating_sub(indent);
    let pad = Span::raw(" ".repeat(indent));
    let mut out = Vec::new();
    match fitted {
        Some(piece) => {
            out.push(Line::from(vec![
                Span::styled(label("■"), fg(CYAN)),
                Span::styled(
                    cut(&piece.name, room),
                    fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]));
            for line in wrap_spans(effects(c, piece, true), room) {
                let mut spans = vec![pad.clone()];
                spans.extend(line.spans);
                out.push(Line::from(spans));
            }
        }
        None => {
            out.push(Line::from(vec![
                Span::styled(label("□"), fg(DIM)),
                Span::styled("empty", fg(FAINTER)),
            ]));
            let takes: Vec<&str> = c
                .items
                .iter()
                .filter(|i| i.implant.is_some_and(|m| m.fits == Fits::Socket(kind)))
                .map(|i| i.name.as_str())
                .collect();
            if !takes.is_empty() {
                out.push(Line::from(vec![
                    pad,
                    Span::styled(cut(&takes.join(", "), room), fg(FAINTER)),
                ]));
            }
        }
    }
    out
}

/// A site's or socket's FITS card: its name, what it is or what it holds, and what in
/// the pack fits it; while the picker is open, the pick on the bar, what it does, what
/// it does to the load and EMP (what it swaps out counted), and its description.
fn fit_card(st: &AppState, v: &View, focused: bool, w: usize) -> CardText {
    let (c, ch) = (&st.content, &st.character);
    let (site, socket) = v.at;
    let mut card = CardText::default();
    let fg = |c: Color| Style::default().fg(c);
    let bold = |c: Color| fg(c).add_modifier(Modifier::BOLD);
    let title = match socket {
        None => site.long().to_string(),
        Some(n) => socket_word(st, site, n).to_uppercase(),
    };
    card.push(Line::from(Span::styled(title, bold(CYAN))));
    match (socket, &v.held) {
        (_, Some(id)) => {
            let name = c
                .item(id)
                .map_or_else(|| id.to_string(), |i| i.name.clone());
            card.extend(wrap_spans(
                vec![
                    Span::styled("replaces ", fg(DIM)),
                    Span::styled(name, fg(TEXT)),
                ],
                w,
            ));
        }
        (None, None) => card.push(Line::from(vec![
            Span::styled("meat", fg(DIM)),
            Span::styled(" · nothing fitted", fg(FAINTER)),
        ])),
        (Some(_), None) => card.extend(wrap_spans(
            vec![
                Span::styled("a socket on the ", fg(DIM)),
                Span::styled(host_name(st, site), fg(TEXT)),
            ],
            w,
        )),
    }
    card.blank();
    // FITS ──── from your bag
    let tail = "from your bag";
    let run = w.saturating_sub("FITS".len() + tail.len() + 2);
    card.push(Line::from(vec![
        Span::styled("FITS", bold(if focused { CYAN } else { DIM })),
        Span::raw(" "),
        Span::styled("─".repeat(run), fg(RULE)),
        Span::raw(" "),
        Span::styled(tail, fg(FAINTER)),
    ]));
    if v.picks.is_empty() {
        card.push(Line::from(Span::styled(
            "nothing in your bag fits",
            fg(FAINTER),
        )));
        return card;
    }
    let (load, max) = (ch.chrome_load(c), ch.load_max());
    for (i, pick) in v.picks.iter().enumerate() {
        let on = v.pick == Some(i);
        let over = load_after(st, v.at, pick) > max;
        let bar = |s: Style| if on { s.bg(BAR_BG) } else { s };
        let name = c
            .item(&pick.item)
            .map_or_else(|| pick.item.to_string(), |i| i.name.clone());
        let tag = format!("L{}", pick.load);
        // `▸ Name ········· L3 `: the tag ends a column short of the edge.
        let name = cut(&name, w.saturating_sub(tag.len() + 4));
        let pad = w.saturating_sub(2 + name.chars().count() + tag.len() + 1);
        let tag_style = match (over, on) {
            (true, _) => fg(MAGENTA),
            (false, true) => fg(lit(st.tick)),
            (false, false) => fg(DIM),
        };
        let lift = |s: Style| {
            if on {
                s.add_modifier(Modifier::BOLD)
            } else {
                s
            }
        };
        card.lines.push(Line::from(vec![
            Span::styled(if on { "▸ " } else { "  " }, bar(bold(lit(st.tick)))),
            Span::styled(name, bar(if on { bold(CYAN) } else { fg(TEXT) })),
            Span::styled(" ".repeat(pad), bar(Style::default())),
            Span::styled(tag, bar(lift(tag_style))),
            Span::styled(" ", bar(Style::default())),
        ]));
        card.bars.push(on);
    }
    let Some(pick) = v.pick.map(|i| &v.picks[i]) else {
        return card;
    };
    let Some(item) = c.item(&pick.item) else {
        return card;
    };
    card.blank();
    card.extend(wrap_spans(effects(c, item, false), w));
    // `load 19 → 21/24 · EMP −2`: what leaves counted; a lighter swap's total plain,
    // and EMP only when the count of pieces moves.
    let after = load_after(st, v.at, pick);
    let pieces = pick_pieces(st, pick) as i32 - held_pieces(st, v.at) as i32;
    let total = match after {
        a if a > max => MAGENTA,
        a if a > load => AMBER,
        _ => TEXT,
    };
    let mut spans = vec![
        Span::styled(format!("load {load} → "), fg(DIM)),
        Span::styled(after.to_string(), bold(total)),
        Span::styled(format!("/{max}"), fg(DIM)),
    ];
    if pieces != 0 {
        let emp = -EMP_PER_PIECE * pieces;
        spans.push(Span::styled(" · ", fg(DIM)));
        spans.push(Span::styled(
            format!("EMP {}", signed(emp)),
            fg(if emp < 0 { MAGENTA } else { GREEN }),
        ));
    }
    card.extend(wrap_spans(spans, w));
    card.description(&item.desc, w);
    card
}

/// `+3`, or `−2` with a true minus.
fn signed(n: i32) -> String {
    if n < 0 {
        format!("−{}", -n)
    } else {
        format!("+{n}")
    }
}

/// A perk in the card's short form, `BODY +1`, `sight +4`, `KIN +15`, and which way it
/// goes.
fn perk_text(p: &Perk) -> (String, i32) {
    let (word, n): (&str, i32) = match *p {
        Perk::Attribute(s, n) => (s.name(), n.into()),
        Perk::Sight(n) => ("sight", n.into()),
        Perk::Carry(kg) => ("carry", kg.round() as i32),
        Perk::HpMax(n) => ("HP", n),
        Perk::Focus(n) => ("focus", n),
        Perk::Armor(n) => ("ARM", n.into()),
        Perk::Move(n) => ("MOV", n.into()),
        Perk::Resist(h, n) => (h.short(), n.into()),
        Perk::Throw(n) => ("throw", n.into()),
    };
    (format!("{word} {}", signed(n)), n)
}

/// What a piece does: its perks, green up and magenta down, and the abilities it grants
/// in cyan, ` · ` between. In full the grants follow the perks after `grants`; compact,
/// under a socket, they lead, bare.
fn effects(c: &Content, item: &Item, compact: bool) -> Vec<Span<'static>> {
    let fg = |c: Color| Style::default().fg(c);
    let perks = item.perks.iter().map(|p| {
        let (text, n) = perk_text(p);
        vec![Span::styled(text, fg(if n < 0 { MAGENTA } else { GREEN }))]
    });
    let mut grants = Vec::new();
    for (i, a) in item.grants.iter().enumerate() {
        if i > 0 {
            grants.push(Span::styled(", ", fg(DIM)));
        }
        let name = c
            .ability(a)
            .map_or_else(|| a.to_string(), |a| a.name.clone());
        grants.push(Span::styled(name, fg(CYAN)));
    }
    let mut parts: Vec<Vec<Span<'static>>> = Vec::new();
    if compact && !grants.is_empty() {
        parts.push(grants.clone());
    }
    parts.extend(perks);
    if !compact && !grants.is_empty() {
        grants.insert(0, Span::styled("grants ", fg(DIM)));
        parts.push(grants);
    }
    let mut out = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            out.push(Span::styled(" · ", fg(DIM)));
        }
        out.extend(part);
    }
    out
}

/// Spans wrapped at their spaces into lines at most `width` wide, each keeping its
/// colours; a word too long for a line is cut.
fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Line<'static>> {
    let chars: Vec<(char, Style)> = spans
        .iter()
        .flat_map(|s| s.content.chars().map(move |ch| (ch, s.style)))
        .collect();
    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    for word in chars.split(|(ch, _)| *ch == ' ').filter(|w| !w.is_empty()) {
        if !row.is_empty() && row.len() + 1 + word.len() > width {
            rows.push(std::mem::take(&mut row));
        }
        if let Some(&(_, style)) = row.last() {
            row.push((' ', style));
        }
        let room = width.saturating_sub(row.len());
        row.extend(word.iter().take(room));
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows.into_iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut run = String::new();
            let mut style = None;
            for (ch, s) in row {
                if style.is_some_and(|st| st != s) {
                    spans.push(Span::styled(
                        std::mem::take(&mut run),
                        style.unwrap_or_default(),
                    ));
                }
                style = Some(s);
                run.push(ch);
            }
            if !run.is_empty() {
                spans.push(Span::styled(run, style.unwrap_or_default()));
            }
            Line::from(spans)
        })
        .collect()
}

/// Draws a card's typed lines into `area`, a row each, its bar rows lit across.
fn draw_lines(buf: &mut Buffer, area: Rect, lines: &[Line<'static>], bars: &[bool]) {
    for (k, line) in lines.iter().take(area.height as usize).enumerate() {
        let y = area.y + k as u16;
        if bars.get(k).copied().unwrap_or(false) {
            for x in area.x..area.right() {
                buf[(x, y)].set_bg(BAR_BG);
            }
        }
        let mut x = area.x;
        for span in &line.spans {
            put(buf, x, y, &span.content, span.style, area.right());
            x = x.saturating_add(span.width() as u16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{JACKED_IN, perform};
    use crate::app::{Screen, ScreenCmd};
    use crate::messages::Kind;
    use crate::screens::deck::{Deck, REVEAL_TICKS};
    use crate::screens::testutil::key;
    use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, RULE, TEXT};
    use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
    use game::{Action, Harm, Stack};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    /// The deck's body at 100×30.
    const BODY: Rect = Rect::new(1, 5, 98, 22);
    /// The right pane's first column at both sizes; at 100×30 the ledger's rule and
    /// the card's first column.
    const R0: u16 = 48;
    const RULE_100: u16 = 66;
    const CARD_100: u16 = 68;

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    /// One of `item` into the pack; its copy when it is held per copy.
    fn give(st: &mut AppState, item: &str) -> Option<CopyId> {
        st.character.give_item(&st.content, id(item), 1);
        st.character
            .inventory
            .iter()
            .rev()
            .find(|s| s.item == id(item))
            .and_then(|s| s.copy_id())
    }

    /// Installs `item` at `site`, as a copy when it is held per copy, with `fitted` in
    /// its sockets.
    fn install(st: &mut AppState, site: Site, item: &str, fitted: &[(usize, &str)]) {
        let per_copy = st.content.item(&id(item)).unwrap().per_copy();
        let mut copy = None;
        if per_copy {
            let mut k = st.character.new_copy(&st.content, &id(item));
            for &(n, piece) in fitted {
                let pc = st.character.new_copy(&st.content, &id(piece));
                k.fitted[n] = Some(Stack {
                    item: id(piece),
                    qty: 1,
                    copy: Some(pc),
                });
            }
            copy = Some(k);
        }
        st.character.install(site, id(item), copy);
    }

    /// The spec's loadout: a piece at every site but EARS, R ARM and LEGS, and the
    /// Piston arm at L ARM with the Arc hand in its hand.
    fn loadout(st: &mut AppState) {
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Back, "drone_cradle"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
        ] {
            install(st, site, item, &[]);
        }
        install(st, Site::LeftArm, "piston_arm", &[(0, "arc_hand")]);
    }

    /// Jacked in: the avatar in a construct, the body on the street.
    fn jacked() -> AppState {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let b = city.buildings[0].id;
        let mut p = crate::walker::Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
        st.city = Some(city);
        st
    }

    /// The deck on ATTRIBUTES with the cursor on the ledger's `row`.
    fn on_ledger(st: &mut AppState, row: LedgerRow) -> Deck {
        let mut d = Deck::new(st.tick);
        d.update(st, &[key(KeyCode::Char('2'))]);
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = row;
        d
    }

    #[test]
    fn the_picker_moves_without_retyping_the_card() {
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        give(&mut st, "surgeon_arm");
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        // The place's card types out once.
        st.tick = d.sheet.card + REVEAL_TICKS + 1;
        let typed = d.sheet.card;
        let now = |d: &Deck, st: &AppState| {
            let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
            t.draw(|f| d.draw(st, f)).unwrap();
            region(t.backend().buffer(), (CARD_100, 98), (6, 27))
        };
        let line = |st: &AppState, item: &str| {
            let desc = &st.content.item(&id(item)).unwrap().desc;
            desc.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        };
        let (grip, surgeon) = (line(&st, "grip_arm"), line(&st, "surgeon_arm"));
        // Opening the picker, moving the pick and closing it keep the card as typed:
        // the pick's lines change in place, whole, at once.
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        assert!(now(&d, &st).contains(&grip), "opened:\n{}", now(&d, &st));
        press(&mut d, &mut st, &[KeyCode::Down]);
        assert_eq!(d.sheet.state.picker, Some(1));
        assert!(now(&d, &st).contains(&surgeon), "moved:\n{}", now(&d, &st));
        press(&mut d, &mut st, &[KeyCode::Up, KeyCode::Esc]);
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.card, typed, "the card never started over");
    }

    fn press(d: &mut Deck, st: &mut AppState, codes: &[KeyCode]) {
        let keys: Vec<_> = codes.iter().map(|&c| key(c)).collect();
        d.update(st, &keys);
    }

    /// The deck at `w`×`h` once its cards and the strip have typed out, clear of the
    /// deck's signal jitter.
    fn screen(d: &Deck, st: &mut AppState, (w, h): (u16, u16)) -> Buffer {
        let typed = d.sheet.card.max(d.sheet.changed) + REVEAL_TICKS + 1;
        st.tick = (st.tick + 25).max(typed);
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    /// `n` cells of row `y` from column `x`.
    fn cells(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
    }

    /// Columns `x0..x1` of rows `y0..y1`, a line each.
    fn region(buf: &Buffer, (x0, x1): (u16, u16), (y0, y1): (u16, u16)) -> String {
        (y0..y1)
            .map(|y| cells(buf, x0, y, x1 - x0).trim_end().to_string() + "\n")
            .collect()
    }

    /// The column in row `y`, from `x0`, where `needle` starts.
    fn col_of(buf: &Buffer, y: u16, x0: u16, needle: &str) -> Option<u16> {
        let want: Vec<String> = needle.chars().map(String::from).collect();
        let n = want.len() as u16;
        (x0..buf.area.right().saturating_sub(n))
            .find(|&x| (0..n).all(|k| buf[(x + k, y)].symbol() == want[k as usize]))
    }

    /// The row, `y0..y1`, whose cells from `x0` hold `needle`, and where it starts.
    fn find(buf: &Buffer, x0: u16, (y0, y1): (u16, u16), needle: &str) -> Option<(u16, u16)> {
        (y0..y1).find_map(|y| col_of(buf, y, x0, needle).map(|x| (x, y)))
    }

    #[test]
    fn ledger_lists_sites_and_sockets() {
        let mut st = AppState::new();
        loadout(&mut st);
        use LedgerRow::{Group, Socket};
        let s = LedgerRow::Site;
        assert_eq!(
            ledger_rows(&st),
            [
                Group("head"),
                s(Site::Neural),
                s(Site::Eyes),
                s(Site::Ears),
                s(Site::Jaw),
                Group("body"),
                s(Site::Spine),
                s(Site::Back),
                s(Site::Organs),
                s(Site::Skin),
                Group("limbs"),
                s(Site::LeftArm),
                Socket(Site::LeftArm, 0),
                Socket(Site::LeftArm, 1),
                s(Site::RightArm),
                s(Site::Legs),
            ]
        );
        let d = on_ledger(&mut st, s(Site::LeftArm));
        let buf = screen(&d, &mut st, (100, 30));
        let ledger = region(&buf, (R0, RULE_100), (6, 27));
        for want in [
            "── head ──",
            "Neural port",
            "· ears",
            "── limbs ──",
            "Piston arm",
            "├ Arc hand",
            "└ □ forearm",
            "· r arm",
            "· legs",
        ] {
            assert!(ledger.contains(want), "{want}:\n{ledger}");
        }
        // A blank row under the CHROME header, across the ledger and the card, as under
        // the left pane's headers.
        assert!(
            cells(&buf, R0, 6, RULE_100 - R0).trim().is_empty(),
            "the ledger's gap"
        );
        assert!(
            cells(&buf, CARD_100, 6, 98 - CARD_100).trim().is_empty(),
            "the card's gap"
        );
        // The mock's columns: groups from R0 + 1, names from R0 + 2, sockets' rules at
        // R0 + 3 and their names at R0 + 5; the cursor's mark at R0 on the bar.
        assert_eq!(cells(&buf, R0 + 1, 7, 10), "── head ──");
        assert_eq!(buf[(R0 + 1, 7)].fg, FAINTER);
        assert_eq!(cells(&buf, R0 + 2, 8, 11), "Neural port");
        assert_eq!(buf[(R0 + 2, 8)].fg, TEXT);
        assert_eq!(cells(&buf, R0 + 2, 10, 6), "· ears");
        assert_eq!(buf[(R0 + 2, 10)].fg, FAINTER);
        assert_eq!(buf[(R0, 18)].symbol(), "▸");
        assert_eq!(cells(&buf, R0 + 2, 18, 10), "Piston arm");
        assert_eq!(buf[(R0 + 2, 18)].fg, CYAN);
        for x in R0..RULE_100 {
            assert_eq!(buf[(x, 18)].bg, BAR_BG, "the cursor's bar at {x}");
        }
        assert_eq!(cells(&buf, R0 + 3, 19, 10), "├ Arc hand");
        assert_eq!(buf[(R0 + 3, 19)].fg, RULE);
        assert_eq!(buf[(R0 + 5, 19)].fg, DIM);
        assert_eq!(cells(&buf, R0 + 3, 20, 11), "└ □ forearm");
        assert_eq!(buf[(R0 + 5, 20)].fg, FAINTER);
        for y in 6..BODY.bottom() {
            assert_eq!(buf[(RULE_100, y)].symbol(), "│", "the ledger's rule at {y}");
        }
        // The header: CHROME, and LOAD with the numbers from content.
        let (load, max) = (
            st.character.chrome_load(&st.content),
            st.character.load_max(),
        );
        assert_eq!(cells(&buf, R0, 5, 6), "CHROME");
        assert_eq!(buf[(R0, 5)].fg, CYAN);
        let header = cells(&buf, R0, 5, 50);
        assert!(header.contains("LOAD"), "{header}");
        let total = format!("{load}/{max}");
        assert!(header.trim_end().ends_with(&total), "{header}");
        assert_eq!(
            cells(&buf, 97, 5, 1),
            max.to_string().chars().last().unwrap().to_string()
        );
        // Twelve segments, the load's share rounded up.
        let filled = (load * 12).div_ceil(max) as usize;
        assert_eq!(header.matches('▰').count(), filled, "{header}");
        assert_eq!(header.matches('▱').count(), 12 - filled, "{header}");
    }

    #[test]
    fn full_body_fits_at_100x30() {
        let mut st = AppState::new();
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Ears, "damper_cochlea"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Back, "drone_cradle"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
            (Site::Legs, "dog_legs"),
        ] {
            install(&mut st, site, item, &[]);
        }
        install(
            &mut st,
            Site::LeftArm,
            "piston_arm",
            &[(0, "arc_hand"), (1, "monowire_spool")],
        );
        install(
            &mut st,
            Site::RightArm,
            "grip_arm",
            &[(0, "toolfingers"), (1, "launcher_sleeve")],
        );
        let rows = ledger_rows(&st);
        assert_eq!(rows.len(), 3 + 11 + 4);
        // Every row on its own line inside the body, whichever the cursor is on.
        for (i, &row) in rows.iter().enumerate() {
            let Some(at) = (match row {
                LedgerRow::Group(_) => None,
                LedgerRow::Site(s) => Some((s, None)),
                LedgerRow::Socket(s, n) => Some((s, Some(n))),
            }) else {
                continue;
            };
            let d = on_ledger(&mut st, row);
            let buf = screen(&d, &mut st, (100, 30));
            let y = BODY.y + 2 + i as u16;
            assert!(y < BODY.bottom());
            assert_eq!(buf[(R0, y)].symbol(), "▸", "{row:?}");
            let name = match at {
                (s, None) => st.character.chrome_at(s).cloned(),
                (s, Some(n)) => st
                    .character
                    .chrome_copy(s)
                    .and_then(|k| k.fitted[n].as_ref().map(|f| f.item.clone())),
            };
            let name = st.content.item(&name.unwrap()).unwrap().name.clone();
            let shown = cells(&buf, R0, y, RULE_100 - R0);
            let lead: String = name.chars().take(8).collect();
            assert!(shown.contains(&lead), "{row:?}: {shown}");
            // The frame's edge right of the body is the deck's.
            for y in BODY.y..BODY.bottom() {
                assert_eq!(buf[(BODY.right(), y)].symbol(), "│", "{row:?} y {y}");
            }
        }
        // Nothing of the sheet outside its body, whichever row.
        for &row in &rows {
            let mut s = Sheet::new(0);
            s.state.pane = SheetPane::Ledger;
            s.state.row = row;
            let mut st2 = AppState::new();
            st2.character = st.character.clone();
            st2.tick = REVEAL_TICKS + 1;
            let area = Rect::new(0, 0, 100, 30);
            let mut buf = Buffer::empty(area);
            for c in &mut buf.content {
                c.set_char('X');
            }
            s.draw(&st2, BODY, &mut buf);
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    if !BODY.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)].symbol(), "X", "{row:?} ({x},{y})");
                    }
                }
            }
        }
    }

    #[test]
    fn card_on_a_limb() {
        let mut st = AppState::new();
        loadout(&mut st);
        let d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in [
            "PISTON ARM",
            "L ARM · load 3 · EMP −2",
            "BODY +1",
            "Shove",
            "■ hand",
            "Arc hand",
            "□ forearm",
        ] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        // The drawing at the top, a blank row, then the name.
        assert_eq!(cells(&buf, CARD_100, 14, 10), "PISTON ARM");
        assert_eq!(buf[(CARD_100, 14)].fg, CYAN);
        let elbow = Color::Rgb(0, 229, 255);
        let lit = (7..13)
            .any(|y| (CARD_100..98).any(|x| buf[(x, y)].fg == elbow || buf[(x, y)].bg == elbow));
        assert!(lit, "the elbow's light in the art's rows");
        let (x, y) = find(&buf, CARD_100, (6, 27), "EMP −2").unwrap();
        assert_eq!(buf[(x, y)].fg, MAGENTA);
        let (x, y) = find(&buf, CARD_100, (6, 27), "BODY +1").unwrap();
        assert_eq!(buf[(x, y)].fg, GREEN);
        let (x, y) = find(&buf, CARD_100, (6, 27), "Shove").unwrap();
        assert_eq!(buf[(x, y)].fg, CYAN);
        // Under the empty forearm, in FAINT, the pieces in content that fit it.
        let forearm: Vec<String> = st
            .content
            .items
            .iter()
            .filter(|i| {
                i.implant
                    .is_some_and(|m| m.fits == Fits::Socket(SocketKind::Forearm))
            })
            .map(|i| i.name.clone())
            .collect();
        assert!(!forearm.is_empty());
        let (x, y) = forearm
            .iter()
            .find_map(|n| find(&buf, CARD_100, (6, 27), n))
            .unwrap_or_else(|| panic!("a forearm piece named:\n{card}"));
        assert_eq!(buf[(x, y)].fg, FAINTER);
    }

    #[test]
    fn picker_installs() {
        let mut st = AppState::new();
        loadout(&mut st);
        let grip = give(&mut st, "grip_arm");
        let (load, max) = (
            st.character.chrome_load(&st.content),
            st.character.load_max(),
        );
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in ["RIGHT ARM", "FITS", "from your bag", "Grip arm", "L3"] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        // The pick on the bar; the ledger's cursor dim.
        let (x, y) = find(&buf, CARD_100, (6, 27), "Grip arm").unwrap();
        assert_eq!(buf[(x, y)].bg, BAR_BG);
        assert_eq!(buf[(R0, 21)].symbol(), "▸");
        assert_eq!(buf[(R0, 21)].fg, DIM);
        assert_ne!(buf[(R0 + 2, 21)].bg, BAR_BG);
        // LOAD shows what it takes, in amber.
        let after = format!(" {}/{max}", load + 3);
        let x = col_of(&buf, 5, R0, &after).unwrap_or_else(|| panic!("{}", cells(&buf, R0, 5, 50)));
        assert_eq!(buf[(x + 1, 5)].fg, AMBER);
        assert!((R0..98).any(|x| buf[(x, 5)].symbol() == "▰" && buf[(x, 5)].fg == AMBER));
        // Space installs it, and the picker closes on the arm.
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(
            st.character.chrome_at(Site::RightArm),
            Some(&id("grip_arm"))
        );
        assert_eq!(st.character.chrome_copy(Site::RightArm).map(|k| k.id), grip);
        assert_eq!(last(&st), Some("installed Grip arm · R ARM"));
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));

        // Esc closes the picker and changes nothing; esc again closes the deck.
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let before = st.character.clone();
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(st.character, before);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn picker_over_budget() {
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        st.character.attributes.cool = 3;
        let (load, max) = (
            st.character.chrome_load(&st.content),
            st.character.load_max(),
        );
        assert!(load <= max && load + 3 > max, "{load} of {max}");
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let buf = screen(&d, &mut st, (100, 30));
        let (_, y) = find(&buf, CARD_100, (6, 27), "Grip arm").unwrap();
        let x = col_of(&buf, y, CARD_100, "L3").unwrap();
        assert_eq!(buf[(x, y)].fg, MAGENTA);
        let before = st.character.clone();
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(st.character, before);
        let why = format!("too much chrome: load {} of {max}", load + 3);
        assert_eq!(last(&st), Some(why.as_str()));
        assert_eq!(st.log.latest().map(|e| e.kind), Some(Kind::Warn));
        assert_eq!(d.sheet.state.picker, Some(0), "still choosing");
    }

    #[test]
    fn picker_closes_when_nothing_fits() {
        let mut st = AppState::new();
        loadout(&mut st);
        st.character.chrome.remove(&Site::LeftArm);
        st.character.chrome_copies.remove(&Site::LeftArm);
        let grip = give(&mut st, "grip_arm").unwrap();
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        // The arm goes in at L ARM by another way: its sockets push R ARM down.
        perform(
            &mut st,
            Action::InstallCopy {
                copy: grip,
                site: Site::LeftArm,
            },
        )
        .unwrap();
        d.update(&mut st, &[]);
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));
        let rows = ledger_rows(&st);
        let i = rows
            .iter()
            .position(|r| *r == LedgerRow::Site(Site::RightArm))
            .unwrap();
        let y = BODY.y + 2 + i as u16;
        let buf = screen(&d, &mut st, (100, 30));
        assert_eq!(buf[(R0, y)].symbol(), "▸");
        assert_eq!(cells(&buf, R0 + 2, y, 7), "· r arm");
        assert!(d.sheet.hints(&st).contains("space what fits"));
    }

    #[test]
    fn nothing_fits_says_so() {
        let mut st = AppState::new();
        loadout(&mut st);
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::Ears));
        let before = st.character.clone();
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(st.character, before);
        assert_eq!(last(&st), Some("nothing in your bag fits"));
        let buf = screen(&d, &mut st, (100, 30));
        assert!(cells(&buf, 2, 27, 60).contains("nothing in your bag fits"));
        // The card says so too, faint, under its FITS.
        let (x, y) = find(&buf, CARD_100, (6, 27), "nothing in your bag fits").unwrap();
        assert_eq!(buf[(x, y)].fg, FAINTER);
        assert!(find(&buf, CARD_100, (6, 27), "FITS").is_some());
    }

    #[test]
    fn pull_and_reinstall_keeps_pieces() {
        let mut st = AppState::new();
        loadout(&mut st);
        let c = st.content.clone();
        let (load, emp) = (
            st.character.chrome_load(&c),
            st.character.resist(&c, Harm::Emp),
        );
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        press(&mut d, &mut st, &[KeyCode::Char('x')]);
        assert_eq!(st.character.chrome_at(Site::LeftArm), None);
        assert_eq!(last(&st), Some("pulled Piston arm · to the pack"));
        let arm = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == id("piston_arm"))
            .unwrap();
        let hand = arm.copy.as_ref().unwrap().fitted[0].as_ref().unwrap();
        assert_eq!(hand.item, id("arc_hand"), "the hand comes out with the arm");
        // Down to R ARM: what fits is the arm, with its hand's load.
        press(&mut d, &mut st, &[KeyCode::Down]);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));
        let picks = fitting(&st, (Site::RightArm, None));
        assert_eq!(picks.len(), 1);
        assert_eq!(
            (picks[0].item.clone(), picks[0].load),
            (id("piston_arm"), 4)
        );
        press(&mut d, &mut st, &[KeyCode::Char(' '), KeyCode::Char(' ')]);
        assert_eq!(
            st.character.chrome_at(Site::RightArm),
            Some(&id("piston_arm"))
        );
        assert!(
            st.character
                .installed()
                .contains(&(Site::RightArm, Some(0), id("arc_hand")))
        );
        assert_eq!(
            (
                st.character.chrome_load(&c),
                st.character.resist(&c, Harm::Emp)
            ),
            (load, emp)
        );
        let buf = screen(&d, &mut st, (100, 30));
        let ledger = region(&buf, (R0, RULE_100), (6, 27));
        assert!(ledger.contains("Piston arm\n   ├ Arc hand"), "{ledger}");
        assert!(ledger.contains("  · l arm"), "{ledger}");
    }

    #[test]
    fn socket_picker() {
        let mut st = AppState::new();
        loadout(&mut st);
        let spool = give(&mut st, "monowire_spool").unwrap();
        let mut d = on_ledger(&mut st, LedgerRow::Socket(Site::LeftArm, 1));
        assert_eq!(
            fitting(&st, (Site::LeftArm, Some(1))),
            [Pick {
                item: id("monowire_spool"),
                copy: Some(spool),
                load: 2,
            }]
        );
        assert!(fitting(&st, (Site::LeftArm, Some(0))).is_empty());
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in [
            "FOREARM",
            "a socket on the Piston arm",
            "Monowire spool",
            "L2",
        ] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        let fitted = st
            .character
            .chrome_copy(Site::LeftArm)
            .and_then(|k| k.fitted[1].clone())
            .unwrap();
        assert_eq!(
            (fitted.item.clone(), fitted.copy_id()),
            (id("monowire_spool"), Some(spool))
        );
        assert!(
            last(&st).is_some_and(|m| m.contains("fitted")),
            "{:?}",
            last(&st)
        );
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.state.row, LedgerRow::Socket(Site::LeftArm, 1));
        // `x` on a socket's piece takes it out to the pack.
        press(&mut d, &mut st, &[KeyCode::Char('x')]);
        assert!(
            st.character
                .chrome_copy(Site::LeftArm)
                .is_some_and(|k| k.fitted[1].is_none())
        );
        assert!(st.character.copy_in_pack(spool).is_some());
    }

    #[test]
    fn locked_jacked_in() {
        let mut st = jacked();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        let before = st.character.clone();
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        // Space on chrome: no picker, the lock's words; `x` the same.
        for code in [KeyCode::Char(' '), KeyCode::Char('x')] {
            st.say("before");
            press(&mut d, &mut st, &[code]);
            assert_eq!(d.sheet.state.picker, None, "{code:?}");
            assert_eq!(st.character, before, "{code:?}");
            assert_eq!(last(&st), Some(JACKED_IN), "{code:?}");
        }
        let buf = screen(&d, &mut st, (100, 30));
        assert!(cells(&buf, 2, 27, 90).contains(JACKED_IN), "the strip");
        let hint = cells(&buf, 2, 28, 70);
        assert!(
            hint.starts_with("↑↓ select · space what fits · x pull it out · ← attributes"),
            "{hint}"
        );
        assert_eq!(buf[(2, 28)].fg, DIM);
        for keys in ["space what fits", "x pull it out"] {
            let x = col_of(&buf, 28, 2, keys).unwrap();
            let n = keys.chars().count() as u16;
            for k in 0..n {
                assert_eq!(buf[(x + k, 28)].fg, FAINTER, "{keys} at {k}");
            }
            assert_eq!(buf[(x + n + 1, 28)].fg, DIM, "after {keys}");
        }
        // An empty site: no picker, the lock's words.
        press(
            &mut d,
            &mut st,
            &[KeyCode::Down, KeyCode::Down, KeyCode::Down],
        );
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));
        st.say("before");
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(st.character, before);
        assert_eq!(last(&st), Some(JACKED_IN));
        let buf = screen(&d, &mut st, (100, 30));
        let x = col_of(&buf, 28, 2, "space what fits").unwrap();
        assert_eq!(buf[(x, 28)].fg, FAINTER);
    }

    #[test]
    fn wide_ledger_names_sites() {
        let mut st = AppState::new();
        loadout(&mut st);
        let d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        let buf = screen(&d, &mut st, (120, 40));
        // The body is (1, 10, 118, 27): the ledger 27 wide, its rule at 75, the card
        // from 77.
        let ledger = region(&buf, (R0, 75), (11, 37));
        for want in [
            "NEURAL  Neural port",
            "EARS    · meat",
            "L ARM   Piston arm",
        ] {
            assert!(ledger.contains(want), "{want}:\n{ledger}");
        }
        assert_eq!(cells(&buf, R0 + 2, 13, 19), "NEURAL  Neural port");
        assert_eq!(buf[(R0 + 2, 13)].fg, FAINTER);
        assert_eq!(buf[(R0 + 10, 13)].fg, TEXT);
        // A socket's row sits under its piece's name.
        let (x, _) = find(&buf, R0, (11, 37), "├ Arc hand").unwrap();
        assert_eq!(x, R0 + 11);
        for y in 11..37 {
            assert_eq!(buf[(75, y)].symbol(), "│", "y {y}");
            assert_eq!(buf[(76, y)].symbol(), " ", "the gap at {y}");
        }
        assert_eq!(cells(&buf, 77, 19, 10), "PISTON ARM");
        let elbow = Color::Rgb(0, 229, 255);
        let lit =
            (12..18).any(|y| (77..118).any(|x| buf[(x, y)].fg == elbow || buf[(x, y)].bg == elbow));
        assert!(lit, "the art right of the rule");
        let card = region(&buf, (77, 118), (11, 37));
        assert!(card.contains("L ARM · load 3 · EMP −2"), "{card}");
        // The header still spans the pane.
        assert_eq!(cells(&buf, R0, 10, 6), "CHROME");
        assert_eq!(
            cells(&buf, 115, 10, 3),
            format!("/{}", st.character.load_max())
        );
    }

    #[test]
    fn hints_follow_the_focus() {
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        let hint_row = |d: &Deck, st: &mut AppState| cells(&screen(d, st, (100, 30)), 2, 28, 90);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let left =
            "←↑↓→ select · space allocates attribute · → over to the chrome · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), left);
        assert!(hint_row(&d, &mut st).starts_with(left));
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Site(Site::LeftArm);
        let chrome =
            "↑↓ select · space what fits · x pull it out · ← attributes · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), chrome);
        assert!(hint_row(&d, &mut st).starts_with(chrome));
        d.sheet.state.row = LedgerRow::Socket(Site::LeftArm, 0);
        assert_eq!(d.sheet.hints(&st), chrome, "a socket's piece is chrome too");
        d.sheet.state.row = LedgerRow::Site(Site::RightArm);
        let empty = "↑↓ select · space what fits · ← attributes · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), empty);
        assert!(hint_row(&d, &mut st).starts_with(empty));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        let picker = "↑↓ choose · space install · esc back · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), picker);
        let row = hint_row(&d, &mut st);
        assert!(row.starts_with(picker), "{row}");
        let x = col_of(&screen(&d, &mut st, (100, 30)), 28, 2, "space install").unwrap();
        assert_eq!(screen(&d, &mut st, (100, 30))[(x, 28)].fg, DIM);
        // On a place that holds a piece, space swaps, and the hint says so.
        press(&mut d, &mut st, &[KeyCode::Esc]);
        d.sheet.state.row = LedgerRow::Site(Site::LeftArm);
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert!(d.sheet.state.picker.is_some());
        let swap = "↑↓ choose · space swap · esc back · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), swap);
        let row = hint_row(&d, &mut st);
        assert!(row.starts_with(swap), "{row}");
    }

    #[test]
    fn space_on_chrome_opens_what_fits_and_swaps() {
        let mut st = AppState::new();
        loadout(&mut st);
        let grip = give(&mut st, "grip_arm");
        give(&mut st, "surgeon_arm");
        give(&mut st, "ceramic_plating");
        let (load, max) = (
            st.character.chrome_load(&st.content),
            st.character.load_max(),
        );
        assert_eq!((load, max), (19, 24));
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        let picks: Vec<ItemId> = fitting(&st, (Site::LeftArm, None))
            .into_iter()
            .map(|p| p.item)
            .collect();
        assert_eq!(picks, [id("grip_arm"), id("surgeon_arm")], "the arms only");
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        assert_eq!(
            st.character.chrome_at(Site::LeftArm),
            Some(&id("piston_arm"))
        );
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in [
            "LEFT ARM",
            "replaces Piston arm",
            "FITS",
            "Grip arm",
            "Surgeon arm",
            "L3",
            "load 19 → 18/24",
        ] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        assert!(
            !card.contains("PISTON ARM"),
            "the piece's card gave way:\n{card}"
        );
        assert!(!card.contains("nothing fitted"), "{card}");
        let (x, y) = find(&buf, CARD_100, (6, 27), "replaces Piston arm").unwrap();
        assert_eq!(buf[(x, y)].fg, DIM);
        assert_eq!(buf[(x + 9, y)].fg, TEXT);
        // Two pieces out, one in: EMP comes back by two.
        let (x, y) = find(&buf, CARD_100, (6, 27), "EMP +2").unwrap();
        assert_eq!(buf[(x, y)].fg, GREEN);
        // Lighter: LOAD shows no amber and the lower total.
        let header = cells(&buf, R0, 5, 50);
        assert!(header.trim_end().ends_with("18/24"), "{header}");
        assert!((R0..98).all(|x| buf[(x, 5)].fg != AMBER), "{header}");
        assert_eq!(header.matches('▰').count(), (18 * 12_usize).div_ceil(24));
        // Space swaps: the Grip arm in, the Piston arm to the pack with its hand.
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(st.character.chrome_at(Site::LeftArm), Some(&id("grip_arm")));
        assert_eq!(st.character.chrome_copy(Site::LeftArm).map(|k| k.id), grip);
        assert_eq!(
            last(&st),
            Some("installed Grip arm · L ARM · Piston arm to the pack")
        );
        let arm = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == id("piston_arm"))
            .expect("the Piston arm in the pack");
        let hand = arm.copy.as_ref().unwrap().fitted[0].as_ref().unwrap();
        assert_eq!(hand.item, id("arc_hand"), "its hand still in it");
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::LeftArm));
        assert_eq!(st.character.chrome_load(&st.content), 18);
    }

    #[test]
    fn the_same_piece_is_no_swap() {
        // The Target lens at EYES, held without a copy: another one is the same piece.
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "target_lens");
        let eyes = (Site::Eyes, None);
        assert!(fitting(&st, eyes).is_empty(), "{:?}", fitting(&st, eyes));
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::Eyes));
        let before = st.character.clone();
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(st.character, before);
        assert_eq!(last(&st), Some("nothing in your bag fits"));
        // Other eyes still swap in.
        give(&mut st, "nightglass_optics");
        let picks: Vec<ItemId> = fitting(&st, eyes).into_iter().map(|p| p.item).collect();
        assert_eq!(picks, [id("nightglass_optics")]);
        // A limb is held per copy: another Piston arm is another arm, and swaps.
        let other = give(&mut st, "piston_arm");
        assert!(other.is_some());
        let arms: Vec<Option<CopyId>> = fitting(&st, (Site::LeftArm, None))
            .into_iter()
            .map(|p| p.copy)
            .collect();
        assert_eq!(arms, [other]);
    }

    #[test]
    fn the_picker_counts_load_as_the_rule_does() {
        // A budget two short of the loadout's: some picks fit by what leaves, some not.
        let mut st = AppState::new();
        loadout(&mut st);
        st.character.attributes.cool = 3;
        for item in [
            "grip_arm",
            "wrecker_arm",
            "ceramic_plating",
            "grounding_weave",
            "second_heart",
            "filter_liver",
            "secondhand_eyes",
            "nightglass_optics",
            "neural_port",
            "limbic_damper",
            "damper_cochlea",
            "dog_legs",
        ] {
            give(&mut st, item);
        }
        let surgeon = give(&mut st, "surgeon_arm").unwrap();
        let tools = st.character.new_copy(&st.content, &id("toolfingers"));
        let k = st
            .character
            .inventory
            .iter_mut()
            .find(|s| s.copy_id() == Some(surgeon))
            .and_then(|s| s.copy.as_mut())
            .unwrap();
        k.fitted[0] = Some(Stack {
            item: id("toolfingers"),
            qty: 1,
            copy: Some(tools),
        });
        let max = st.character.load_max();
        let mut seen = (0, 0);
        for site in Site::ALL {
            for pick in fitting(&st, (site, None)) {
                let stack = st
                    .character
                    .inventory
                    .iter()
                    .find(|s| match pick.copy {
                        Some(k) => s.copy_id() == Some(k),
                        None => s.copy.is_none() && s.item == pick.item,
                    })
                    .unwrap();
                let after = load_after(&st, (site, None), &pick);
                let rule = st.character.can_install_stack(&st.content, stack, site);
                let over = format!("too much chrome: load {after} of {max}");
                match rule {
                    Ok(()) => assert!(after <= max, "{site:?} {:?}", pick.item),
                    Err(why) => assert_eq!(why, over, "{site:?} {:?}", pick.item),
                }
                if after > max {
                    seen.1 += 1;
                } else {
                    seen.0 += 1;
                }
            }
        }
        assert!(seen.0 > 0 && seen.1 > 0, "fits and refusals both: {seen:?}");
    }

    #[test]
    fn a_heavier_swap_counts_what_leaves() {
        // SKIN: Ceramic plating (4) for the Subdermal mesh (2).
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "ceramic_plating");
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::Skin));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in ["SKIN", "replaces Subdermal mesh", "Ceramic plating", "L4"] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        let (x, y) = find(&buf, CARD_100, (6, 27), "load 19 → 21/24").unwrap();
        assert_eq!(buf[(x + 10, y)].fg, AMBER);
        assert!(
            !card.contains("EMP"),
            "one piece for one: EMP moves not:\n{card}"
        );
        // The gauge: the two more in amber, the total with them.
        let header = cells(&buf, R0, 5, 50);
        assert!(header.trim_end().ends_with("21/24"), "{header}");
        let x = col_of(&buf, 5, R0, " 21/24").unwrap();
        assert_eq!(buf[(x + 1, 5)].fg, AMBER);
        assert!((R0..98).any(|x| buf[(x, 5)].symbol() == "▰" && buf[(x, 5)].fg == AMBER));

        // Over the budget even once the mesh's two leave: magenta, and it refuses.
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "ceramic_plating");
        st.character.attributes.cool = 3;
        assert_eq!(st.character.load_max(), 20);
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::Skin));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        let buf = screen(&d, &mut st, (100, 30));
        let (_, y) = find(&buf, CARD_100, (6, 27), "Ceramic plating").unwrap();
        let x = col_of(&buf, y, CARD_100, "L4").unwrap();
        assert_eq!(buf[(x, y)].fg, MAGENTA);
        let (x, y) = find(&buf, CARD_100, (6, 27), "load 19 → 21/20").unwrap();
        assert_eq!(buf[(x + 10, y)].fg, MAGENTA);
        let before = st.character.clone();
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(st.character, before);
        assert_eq!(last(&st), Some("too much chrome: load 21 of 20"));
        assert_eq!(d.sheet.state.picker, Some(0), "still choosing");
    }

    #[test]
    fn space_on_a_socket_piece_swaps_it() {
        let mut st = AppState::new();
        loadout(&mut st);
        let fingers = give(&mut st, "toolfingers");
        let hand = st
            .character
            .chrome_copy(Site::LeftArm)
            .and_then(|k| k.fitted[0].as_ref()?.copy_id());
        let mut d = on_ledger(&mut st, LedgerRow::Socket(Site::LeftArm, 0));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in [
            "HAND",
            "replaces Arc hand",
            "Toolfingers",
            "L1",
            "load 19 → 19/24",
        ] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        let (x, y) = find(&buf, CARD_100, (6, 27), "load 19 → 19/24").unwrap();
        assert_eq!(buf[(x + 10, y)].fg, TEXT, "nothing more: plain");
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        let fitted = st
            .character
            .chrome_copy(Site::LeftArm)
            .and_then(|k| k.fitted[0].clone())
            .unwrap();
        assert_eq!(fitted.copy_id(), fingers);
        assert_eq!(
            last(&st),
            Some("Toolfingers fitted · socket hand · Arc hand to the pack")
        );
        assert!(hand.is_some_and(|k| st.character.copy_in_pack(k).is_some()));
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.state.row, LedgerRow::Socket(Site::LeftArm, 0));
    }

    #[test]
    fn the_swap_picker_types_its_card_once() {
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        give(&mut st, "surgeon_arm");
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        st.tick = d.sheet.card + REVEAL_TICKS + 1;
        // Opening on a piece: its card gives way to the FITS card, typed out once.
        st.tick += 1;
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        let opened = st.tick;
        assert_eq!(d.sheet.card, opened);
        // Moving the pick never types it again.
        st.tick += 5;
        press(&mut d, &mut st, &[KeyCode::Down]);
        assert_eq!(d.sheet.state.picker, Some(1));
        st.tick += 5;
        press(&mut d, &mut st, &[KeyCode::Up]);
        assert_eq!(d.sheet.card, opened);
        // Esc: the piece's card again, typed out.
        st.tick += 5;
        press(&mut d, &mut st, &[KeyCode::Esc]);
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.card, st.tick);
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        assert!(card.contains("PISTON ARM"), "{card}");
    }

    #[test]
    fn x_pulls_chrome_and_nothing_else() {
        let mut st = AppState::new();
        loadout(&mut st);
        // On an empty place `x` does nothing.
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        st.say("before");
        let before = st.character.clone();
        press(&mut d, &mut st, &[KeyCode::Char('x')]);
        assert_eq!((&st.character, last(&st)), (&before, Some("before")));
        let mut d = on_ledger(&mut st, LedgerRow::Socket(Site::LeftArm, 1));
        press(&mut d, &mut st, &[KeyCode::Char('x')]);
        assert_eq!((&st.character, last(&st)), (&before, Some("before")));
        // On a socket's piece: out to the pack.
        let mut d = on_ledger(&mut st, LedgerRow::Socket(Site::LeftArm, 0));
        press(&mut d, &mut st, &[KeyCode::Char('x')]);
        assert_eq!(last(&st), Some("Arc hand out · to the pack"));
        assert_eq!(st.character.count(&id("arc_hand")), 1);
        // On a site's piece: out to the pack; the card types afresh.
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        st.tick += 7;
        press(&mut d, &mut st, &[KeyCode::Char('x')]);
        assert_eq!(st.character.chrome_at(Site::LeftArm), None);
        assert_eq!(last(&st), Some("pulled Piston arm · to the pack"));
        assert_eq!(d.sheet.card, st.tick);
        assert_eq!(d.sheet.state.picker, None);
    }

    #[test]
    fn ledger_keys_skip_the_groups() {
        let mut st = AppState::new();
        loadout(&mut st);
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::Neural));
        let walk = |d: &mut Deck, st: &mut AppState, code: KeyCode| {
            st.tick += 5;
            press(d, st, &[code]);
            d.sheet.state.row
        };
        let s = LedgerRow::Site;
        assert_eq!(
            walk(&mut d, &mut st, KeyCode::Up),
            s(Site::Neural),
            "the first stops"
        );
        for want in [Site::Eyes, Site::Ears, Site::Jaw, Site::Spine] {
            assert_eq!(walk(&mut d, &mut st, KeyCode::Down), s(want));
        }
        assert_eq!(d.sheet.card, st.tick, "the card types out afresh");
        assert_eq!(
            walk(&mut d, &mut st, KeyCode::Char('k')),
            s(Site::Jaw),
            "over the body's header"
        );
        d.sheet.state.row = s(Site::Skin);
        for want in [
            s(Site::LeftArm),
            LedgerRow::Socket(Site::LeftArm, 0),
            LedgerRow::Socket(Site::LeftArm, 1),
            s(Site::RightArm),
            s(Site::Legs),
            s(Site::Legs),
        ] {
            assert_eq!(walk(&mut d, &mut st, KeyCode::Char('j')), want);
        }
        // Right does nothing; left goes back to the tile the left pane had.
        let tile = d.sheet.state.tile;
        walk(&mut d, &mut st, KeyCode::Right);
        assert_eq!(d.sheet.state.pane, SheetPane::Ledger);
        walk(&mut d, &mut st, KeyCode::Left);
        assert_eq!(
            (d.sheet.state.pane, d.sheet.state.tile),
            (SheetPane::Left, tile)
        );
        assert_eq!(d.sheet.state.row, s(Site::Legs), "the ledger keeps its row");
    }

    #[test]
    fn a_tall_card_gives_way_blanks_then_description() {
        let card = || {
            let mut card = CardText::default();
            for t in ["NAME", "", "a", "b", ""] {
                card.push(Line::from(t));
            }
            card.desc = Some(card.lines.len());
            for t in ["one two", "three", "four"] {
                card.push(Line::from(t));
            }
            card
        };
        let shown = |rows: usize| -> Vec<String> {
            let c = card().fit(rows, 20);
            assert_eq!(c.lines.len(), c.bars.len());
            c.lines.iter().map(|l| l.to_string()).collect()
        };
        assert_eq!(
            shown(8),
            ["NAME", "", "a", "b", "", "one two", "three", "four"]
        );
        assert_eq!(shown(7), ["NAME", "a", "b", "", "one two", "three", "four"]);
        assert_eq!(shown(6), ["NAME", "a", "b", "one two", "three", "four"]);
        assert_eq!(shown(5), ["NAME", "a", "b", "one two", "three…"]);
        assert_eq!(shown(3), ["NAME", "a", "b"]);
        assert_eq!(shown(2), ["NAME", "a"]);
    }

    #[test]
    fn rows_scroll_with_the_cursor() {
        let mut st = AppState::new();
        loadout(&mut st);
        let mut s = Sheet::new(0);
        s.state.pane = SheetPane::Ledger;
        s.state.row = LedgerRow::Site(Site::Legs);
        st.tick = REVEAL_TICKS + 1;
        let body = Rect::new(1, 5, 98, 10);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        s.draw(&st, body, &mut buf);
        let ledger = region(&buf, (R0, RULE_100), (body.y + 1, body.bottom()));
        assert!(ledger.contains("· legs"), "{ledger}");
        assert!(!ledger.contains("── head ──"), "{ledger}");
        let (_, y) = find(&buf, R0, (body.y + 1, body.bottom()), "· legs").unwrap();
        assert_eq!(buf[(R0, y)].symbol(), "▸");
    }
}
