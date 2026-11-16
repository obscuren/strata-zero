//! GEAR & INVENTORY: the body on the left, a tile for each part with what it wears;
//! on the right the pack of what you carry, and under it the card of the item under the
//! cursor, with its drawing at half size when there is room.

pub(crate) mod body;

use crate::app::AppState;
use crate::render::glyphs::kind_icon;
use crate::screens::blueprint::{MISSING, preview_art};
use crate::screens::deck::{
    Detail, REVEAL_TICKS, Row, Tone, card, copy_popup, draw_rows, item_popup, item_row, lit, marks,
    part_popup,
};
use crate::screens::popup::Popup;
use crate::ui::chrome::{DIM, FAINTER, RULE, dim, put, put_paragraph, reveal, solid};
use crate::ui::cyberdeck::Pen;
use crate::ui::preview::half;
use body::{BodyLayout, Dir};
use crossterm::event::KeyCode;
use game::{BodyPart, CopyId, ItemId, ItemKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget, Wrap};
use std::cell::Cell;

/// What the card's corner says an item can do.
const CAPTION: &str = "space inspect";
/// The box that stands in for a drawing not made yet, in columns and rows.
const MISSING_BOX: (u16, u16) = (21, 5);
/// The pack's rows at the full body's least height: the filter's chips and ten rows.
const PACK_ROWS: u16 = 11;
/// Rows the card keeps at least, under the pack.
const CARD_ROWS: u16 = 12;
/// Rows the drawing takes under the card's text.
const VIEW_ROWS: u16 = 8;
/// The tab's body before its first draw, as a 120×40 window has it: what the arrows
/// walk until the tab has drawn.
const UNDRAWN: Rect = Rect::new(1, 10, 118, 27);

/// Whether the pack holds anything of `kind`.
fn carried(st: &AppState, kind: ItemKind) -> bool {
    st.character
        .inventory
        .iter()
        .any(|s| st.content.item(&s.item).is_some_and(|i| i.kind == kind))
}

/// Lit and bold for the focused selection.
fn lit_bold(tick: u64) -> Style {
    Style::default().fg(lit(tick)).add_modifier(Modifier::BOLD)
}

/// Which side holds the cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Body,
    Pack,
}

/// Where the cursor is, kept between visits to the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitState {
    pub pane: Pane,
    /// The part the body's cursor is on.
    pub part: BodyPart,
    /// The pack's row, headers counted.
    pub row: usize,
    /// The kind the pack shows; `None` is all of them.
    pub filter: Option<ItemKind>,
}

impl Default for KitState {
    fn default() -> Self {
        KitState {
            pane: Pane::Body,
            part: BodyPart::Head,
            row: 0,
            filter: None,
        }
    }
}

/// The tab's parts: the body, the divider right of it, and beside it the pack over a
/// rule with the card under it.
pub(crate) struct Layout {
    pub body: BodyLayout,
    pub divider: u16,
    pub pack: Rect,
    pub rule: u16,
    pub card: Rect,
}

pub(crate) fn layout(area: Rect, parts: &[BodyPart]) -> Layout {
    let body = body::layout(area, parts);
    let divider = body.left.right();
    // The pack's rows start with the cursor's column; the card starts after it.
    let (pack_x, x) = (divider + 1, divider + 2);
    // Ten rows of pack at the full body's height, fewer below it; half of any more.
    let pack_h = area.height.saturating_sub(CARD_ROWS + 1).min(PACK_ROWS)
        + area.height.saturating_sub(body::FULL_MIN.1) / 2;
    let rule = area.y + pack_h;
    Layout {
        body,
        divider,
        pack: Rect::new(pack_x, area.y, area.right().saturating_sub(pack_x), pack_h),
        rule,
        card: Rect::new(
            x,
            rule + 1,
            area.right().saturating_sub(x + 1),
            area.bottom().saturating_sub(rule + 1),
        ),
    }
}

/// The body's parts in body order: its tiles.
fn parts(st: &AppState) -> Vec<BodyPart> {
    let body = st.character.body(&st.content);
    body.into_iter().map(|n| n.part).collect()
}

/// Rows `lines` take wrapped in `width` columns, as the card draws them.
fn rows_of(lines: &[Line<'static>], width: u16) -> u16 {
    if width == 0 {
        return 0;
    }
    let area = Rect::new(0, 0, width, 200);
    let mut buf = Buffer::empty(area);
    Paragraph::new(lines.to_vec())
        .wrap(Wrap { trim: false })
        .render(area, &mut buf);
    (0..area.height)
        .rev()
        .find(|&y| (0..width).any(|x| buf[(x, y)].symbol() != " "))
        .map_or(0, |y| y + 1)
}

/// An item's card without its closing description (the blank line and the words
/// after it), which Inspect shows; any other card as it is.
fn without_blurb<'a>(lines: &'a [Line<'static>]) -> &'a [Line<'static>] {
    match lines {
        [rest @ .., gap, _] if gap.width() == 0 => rest,
        all => all,
    }
}

pub struct Kit {
    pub state: KitState,
    /// Tick of the last selection or pane change; drives the card's typewriter.
    pub changed: u64,
    /// Columns a pack row's text had at the last draw, after the cursor's `▸ `.
    room: Cell<usize>,
    /// The tab's body at the last draw, to place the tiles for the arrows and popups.
    area: Cell<Rect>,
    /// The pack's first visible row at the last draw.
    first: Cell<usize>,
}

impl Kit {
    pub fn new(tick: u64) -> Kit {
        Kit {
            state: KitState::default(),
            changed: tick,
            room: Cell::new(usize::MAX),
            area: Cell::new(Rect::default()),
            first: Cell::new(0),
        }
    }

    /// Reopens where the tab was left (`st.deck_kit`), the cursor kept on something
    /// that still exists. The card types out afresh.
    pub fn restore(st: &AppState) -> Kit {
        let mut kit = Kit::new(st.tick);
        kit.state = st.deck_kit;
        kit.reselect(st);
        kit
    }

    /// The tab's body as last drawn, or a 120×40 window's before the first draw.
    fn area(&self) -> Rect {
        let area = self.area.get();
        if area.is_empty() { UNDRAWN } else { area }
    }

    fn layout(&self, st: &AppState) -> Layout {
        layout(self.area(), &parts(st))
    }

    /// The part the cursor is on among the tiles drawn: the one kept, or when the body
    /// doesn't draw it (an arm on the compact body) the drawn part that hangs off it,
    /// else the one it hangs off.
    fn cursor(&self, st: &AppState, l: &BodyLayout) -> BodyPart {
        let part = self.state.part;
        if l.tile(part).is_some() {
            return part;
        }
        let drawn = |p: &BodyPart| l.tile(*p).is_some();
        let body = st.character.body(&st.content);
        let off = body
            .iter()
            .find(|n| n.joins == Some(part) && drawn(&n.part))
            .map(|n| n.part);
        let on = st.content.node(part).and_then(|n| n.joins).filter(drawn);
        off.or(on).or(l.tiles.first().map(|t| t.0)).unwrap_or(part)
    }

    /// The pack, one row per stack under a header for each kind. A kind picked by the
    /// filter shows its stacks alone, without a header.
    pub(crate) fn pack_rows(&self, st: &AppState) -> Vec<Row> {
        let mut rows = Vec::new();
        for kind in ItemKind::ALL {
            if self.state.filter.is_some_and(|f| f != kind) {
                continue;
            }
            let stacks: Vec<_> = st
                .character
                .inventory
                .iter()
                .filter(|s| st.content.item(&s.item).is_some_and(|i| i.kind == kind))
                .collect();
            if stacks.is_empty() {
                continue;
            }
            if self.state.filter.is_none() {
                rows.push(Row {
                    text: format!("── {} ──", kind.label()),
                    tone: Tone::Dim,
                    detail: None,
                    spans: None,
                });
            }
            for s in stacks {
                let item = st.content.item(&s.item).expect("filtered above");
                let lead = format!("{} {:>2}× ", kind_icon(kind), s.qty);
                let selected = self.state.pane == Pane::Pack && rows.len() == self.state.row;
                let marks = marks(st, item, s.copy.as_ref(), false, selected);
                let detail = match s.copy_id() {
                    Some(copy) => Detail::Copy(copy),
                    None => Detail::Item(s.item.clone()),
                };
                rows.push(item_row(
                    st,
                    self.room.get(),
                    lead,
                    &item.name,
                    marks,
                    selected,
                    detail,
                ));
            }
        }
        rows
    }

    /// What the cursor is on: the tile's part, or the pack row's item.
    pub(crate) fn selected(&self, st: &AppState) -> Option<Detail> {
        match self.state.pane {
            Pane::Body => Some(Detail::Part(self.cursor(st, &self.layout(st).body))),
            Pane::Pack => self.pack_rows(st).get(self.state.row)?.detail.clone(),
        }
    }

    fn set_pane(&mut self, pane: Pane, tick: u64) {
        self.state.pane = pane;
        self.changed = tick;
    }

    /// Moves the pack's cursor to the next row with a detail, wrapping at both ends.
    fn move_row(&mut self, st: &AppState, delta: i32, tick: u64) {
        let rows = self.pack_rows(st);
        let n = rows.len() as i32;
        let mut i = self.state.row as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
            if rows[i as usize].detail.is_some() {
                self.state.row = i as usize;
                self.changed = tick;
                return;
            }
        }
    }

    /// Steps the filter through all, then each kind, skipping the kinds not carried.
    fn step_filter(&mut self, st: &AppState, delta: i32, tick: u64) {
        let order: Vec<Option<ItemKind>> = std::iter::once(None)
            .chain(ItemKind::ALL.into_iter().map(Some))
            .collect();
        let n = order.len() as i32;
        let mut i = order
            .iter()
            .position(|f| *f == self.state.filter)
            .unwrap_or(0) as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
            if order[i as usize].is_none_or(|kind| carried(st, kind)) {
                break;
            }
        }
        self.state.filter = order[i as usize];
        self.state.row = self
            .pack_rows(st)
            .iter()
            .position(|r| r.detail.is_some())
            .unwrap_or(0);
        if self.state.pane == Pane::Pack {
            self.changed = tick;
        }
    }

    /// Handles a key; true when it was the kit's. On the body the arrows go from tile
    /// to tile, and right with no tile that way crosses to the pack; on the pack they
    /// move the row, and left comes back to the part left.
    pub(crate) fn key(&mut self, st: &AppState, code: KeyCode, tick: u64) -> bool {
        let dir = match code {
            KeyCode::Char('[') => {
                self.step_filter(st, -1, tick);
                return true;
            }
            KeyCode::Char(']') => {
                self.step_filter(st, 1, tick);
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') => Dir::Up,
            KeyCode::Down | KeyCode::Char('j') => Dir::Down,
            KeyCode::Left | KeyCode::Char('h') => Dir::Left,
            KeyCode::Right | KeyCode::Char('l') => Dir::Right,
            _ => return false,
        };
        match self.state.pane {
            Pane::Body => {
                let l = self.layout(st).body;
                match body::step(&l, self.cursor(st, &l), dir) {
                    Some(part) => {
                        self.state.part = part;
                        self.changed = tick;
                    }
                    None if dir == Dir::Right => self.set_pane(Pane::Pack, tick),
                    None => {}
                }
            }
            Pane::Pack => match dir {
                Dir::Up => self.move_row(st, -1, tick),
                Dir::Down => self.move_row(st, 1, tick),
                Dir::Left => self.set_pane(Pane::Body, tick),
                Dir::Right => {}
            },
        }
        true
    }

    /// Keeps the cursor on something that exists after the gear or the pack changed.
    pub(crate) fn reselect(&mut self, st: &AppState) {
        let parts = parts(st);
        if !parts.contains(&self.state.part) {
            self.state.part = parts.first().copied().unwrap_or(BodyPart::Head);
        }
        let rows = self.pack_rows(st);
        if !rows.get(self.state.row).is_some_and(|r| r.detail.is_some()) {
            self.state.row = rows.iter().position(|r| r.detail.is_some()).unwrap_or(0);
        }
    }

    /// The popup for the selection: a tile's part, or a pack row's item or copy.
    pub(crate) fn interact(&self, st: &mut AppState) -> Option<Popup> {
        let detail = self.selected(st)?;
        let l = self.layout(st);
        match detail {
            Detail::Part(part) => {
                let tile = l.body.tile(part)?;
                part_popup(st, part, (tile.right() + 1, tile.y + 1))
            }
            other => {
                let line = self.state.row.saturating_sub(self.first.get()) as u16;
                let anchor = (l.pack.x + 2, l.pack.y + 1 + line + 1);
                match other {
                    Detail::Item(id) => item_popup(st, &id, anchor),
                    Detail::Copy(copy) => copy_popup(st, copy, anchor),
                    _ => None,
                }
            }
        }
    }

    pub(crate) fn hints(&self) -> &'static str {
        match self.state.pane {
            Pane::Body => {
                "←↑↓→ body, then over to the pack · [ ] filter · space act · Q/E tabs · tab close"
            }
            Pane::Pack => {
                "←→ gear/pack · ↑↓ select · [ ] filter · space act · Q/E tabs · tab close"
            }
        }
    }

    /// Draws the tab into `area`: the body, the divider, the pack over the rule and the
    /// selection's card under it. Keeps the area and the pack's first row for the
    /// arrows and the popups.
    pub(crate) fn draw(&self, st: &AppState, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        self.area.set(area);
        let l = self.layout(st);
        // Rows draw after the cursor's two columns, one in from the pane's edge.
        self.room.set(l.pack.width.saturating_sub(3) as usize);
        let focused = self.state.pane == Pane::Body;
        body::draw(st, &l.body, self.cursor(st, &l.body), focused, buf);
        self.draw_pack(st, l.pack, buf);
        self.draw_rule(st, area, &l, buf);
        self.draw_card(st, area, &l, buf);
    }

    /// The pack: the filter's chips on its first row, ALL then a kind's glyph each, the
    /// active one bracketed and named, the glyphs closer together when the row is
    /// narrow; under them the list.
    fn draw_pack(&self, st: &AppState, pack: Rect, buf: &mut Buffer) {
        let focused = self.state.pane == Pane::Pack;
        let chips = |tight: bool| -> Vec<(Option<ItemKind>, bool, String)> {
            std::iter::once(None)
                .chain(ItemKind::ALL.into_iter().map(Some))
                .map(|chip| {
                    let on = chip == self.state.filter;
                    let text = match (chip, on) {
                        (None, true) => "▌ALL▐".to_string(),
                        (None, false) => " ALL ".to_string(),
                        (Some(kind), true) => {
                            format!("▌{} {}▐", kind_icon(kind), kind.label().to_uppercase())
                        }
                        (Some(kind), false) if tight => format!(" {}", kind_icon(kind)),
                        (Some(kind), false) => format!(" {} ", kind_icon(kind)),
                    };
                    (chip, on, text)
                })
                .collect()
        };
        let wide = chips(false);
        let width: usize = wide.iter().map(|(_, _, t)| t.chars().count()).sum();
        let row = if width < usize::from(pack.width) {
            wide
        } else {
            chips(true)
        };
        let mut x = pack.x + 1;
        for (chip, on, text) in row {
            let style = if on && focused {
                lit_bold(st.tick)
            } else if on || chip.is_none_or(|kind| carried(st, kind)) {
                Style::default().fg(DIM)
            } else {
                Style::default().fg(FAINTER)
            };
            put(buf, x, pack.y, &text, style, pack.right());
            x = x.saturating_add(text.chars().count() as u16);
        }

        let list = Rect::new(
            pack.x,
            pack.y + 1,
            pack.width,
            pack.height.saturating_sub(1),
        );
        let rows = self.pack_rows(st);
        let h = list.height as usize;
        let first = self
            .state
            .row
            .saturating_sub(h.saturating_sub(1) / 2)
            .min(rows.len().saturating_sub(h));
        self.first.set(first);
        draw_rows(buf, list, &rows, first, self.state.row, focused, st.tick);
    }

    /// The divider down the tab, and the rule across the pack's side under the pack
    /// with the weight and worth of everything carried, worn included, at its right end.
    fn draw_rule(&self, st: &AppState, area: Rect, l: &Layout, buf: &mut Buffer) {
        for cy in area.y..area.bottom() {
            buf[(l.divider, cy)].set_char('║').set_fg(RULE);
        }
        for cx in l.divider + 1..area.right() {
            buf[(cx, l.rule)].set_char('═').set_fg(RULE);
        }
        buf[(l.divider, l.rule)].set_char('╠').set_fg(RULE);
        let total = format!(
            " {:.1} kg · ¤ {} ",
            st.character.weight(&st.content),
            st.character.value(&st.content)
        );
        let end = area.right().saturating_sub(2);
        let x = end.saturating_sub(total.chars().count() as u16);
        put(buf, x, l.rule, &total, Style::default().fg(DIM), end);
    }

    /// The selection's card under the rule, typing out, and under its text the drawing
    /// at half size when 8 rows are left for it; an item's description gives way to
    /// the drawing when both don't fit. Beside the compact body, or with no room for
    /// the drawing, `space inspect` alone, in the card's corner.
    fn draw_card(&self, st: &AppState, area: Rect, l: &Layout, buf: &mut Buffer) {
        let selected = self.selected(st);
        let lines = match &selected {
            Some(d) => card(st, d),
            None => vec![dim("nothing here")],
        };
        let shown = selected.as_ref().and_then(|d| shown_item(st, d));
        let fits =
            |lines: &[Line<'static>]| rows_of(lines, l.card.width) + 1 + VIEW_ROWS <= l.card.height;
        let (lines, view) = match &shown {
            None => (lines, false),
            Some(_) if !l.body.full => (lines, false),
            Some(_) if fits(&lines) => (lines, true),
            Some(_) if fits(without_blurb(&lines)) => (without_blurb(&lines).to_vec(), true),
            Some(_) => (lines, false),
        };
        let mut text = l.card;
        if view {
            text.height -= VIEW_ROWS + 1;
        } else if shown.is_some() {
            text.height = text.height.saturating_sub(1);
        }
        let tick = st.tick;
        let p = tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
        put_paragraph(buf, text, reveal(&lines, p, (tick / 4).is_multiple_of(2)));
        match shown {
            Some(item) if view => {
                let y = area.bottom().saturating_sub(VIEW_ROWS);
                let width = area.right().saturating_sub(l.card.x);
                draw_viewer(st, item, Rect::new(l.card.x, y, width, VIEW_ROWS), buf);
            }
            Some(_) => {
                let end = area.right().saturating_sub(2);
                let x = end.saturating_sub(CAPTION.chars().count() as u16);
                let y = area.bottom().saturating_sub(1);
                put(buf, x, y, CAPTION, Style::default().fg(FAINTER), end);
            }
            None => {}
        }
    }
}

/// The item a row's card and the viewer show, whether it is worn, and its copy: the
/// pack's item or copy, or what the part has on.
pub(crate) fn shown_item(st: &AppState, detail: &Detail) -> Option<(ItemId, bool, Option<CopyId>)> {
    match detail {
        Detail::Part(part) => st.character.equipped(*part).map(|id| {
            let copy = st.character.worn_copy(*part).map(|k| k.id);
            (id.clone(), true, copy)
        }),
        Detail::Item(id) => Some((id.clone(), false, None)),
        Detail::Copy(copy) => st
            .character
            .copy_in_pack(*copy)
            .map(|s| (s.item.clone(), false, Some(*copy))),
        _ => None,
    }
}

/// The viewer: a solid window with corner ticks and the item's drawing at half size,
/// centred and clipped to it, or the box that says it has none yet, and `space inspect`
/// in its corner.
fn draw_viewer(
    st: &AppState,
    (item, equipped, copy): (ItemId, bool, Option<CopyId>),
    view: Rect,
    buf: &mut Buffer,
) {
    if view.width < 2 || view.height < 2 {
        return;
    }
    solid(buf, view);
    let (r, b) = (view.right() - 1, view.bottom() - 1);
    let corner = Style::default().fg(RULE);
    for (x, y, ch) in [
        (view.x, view.y, "┌"),
        (r, view.y, "┐"),
        (view.x, b, "└"),
        (r, b, "┘"),
    ] {
        put(buf, x, y, ch, corner, x + 1);
    }
    match preview_art(st, &item, equipped, copy) {
        Some(art) => {
            let px = half(art.as_ref());
            let (cw, ch) = px.cells();
            let x = view.x + view.width.saturating_sub(cw) / 2;
            let y = view.y + view.height.saturating_sub(ch) / 2;
            let mut pen = Pen {
                buf: &mut *buf,
                ox: 0,
                oy: 0,
                clip: view,
            };
            px.fold(&mut pen, i32::from(x), i32::from(y));
        }
        None => {
            let icon = st.content.item(&item).map(|i| kind_icon(i.kind));
            missing_box(buf, view, icon);
        }
    }
    let end = view.right().saturating_sub(2);
    let x = end.saturating_sub(CAPTION.chars().count() as u16);
    put(buf, x, b, CAPTION, Style::default().fg(FAINTER), end);
}

/// A dashed box centred in `view`, the kind's glyph on its second row and the
/// blueprint window's words on its fourth.
fn missing_box(buf: &mut Buffer, view: Rect, icon: Option<char>) {
    let (w, h) = MISSING_BOX;
    let x = view.x + view.width.saturating_sub(w) / 2;
    let y = view.y + view.height.saturating_sub(h) / 2;
    let faint = Style::default().fg(FAINTER);
    let mut at = |cx: u16, cy: u16, text: &str, style: Style| {
        if cy < view.bottom() {
            put(buf, cx, cy, text, style, view.right());
        }
    };
    let (r, b) = (x + w - 1, y + h - 1);
    for cx in x..=r {
        at(cx, y, "╌", faint);
        at(cx, b, "╌", faint);
    }
    for cy in y..=b {
        at(x, cy, "╎", faint);
        at(r, cy, "╎", faint);
    }
    for (cx, cy, ch) in [(x, y, "┌"), (r, y, "┐"), (x, b, "└"), (r, b, "┘")] {
        at(cx, cy, ch, faint);
    }
    if let Some(icon) = icon {
        let bold = Style::default().fg(DIM).add_modifier(Modifier::BOLD);
        at(x + w / 2, y + 1, &icon.to_string(), bold);
    }
    let label_x = x + w.saturating_sub(MISSING.chars().count() as u16) / 2;
    at(label_x, y + 3, MISSING, faint);
}

#[cfg(test)]
mod tests;
