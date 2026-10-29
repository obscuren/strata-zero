//! GEAR & INVENTORY: the rack of what you wear, the pack of what you carry, and the
//! item under the cursor, its card beside its drawing at half size.

use crate::anim::{Easing, Tween};
use crate::app::AppState;
use crate::render::glyphs::kind_icon;
use crate::screens::blueprint::{MISSING, preview_art};
use crate::screens::deck::{
    Detail, REVEAL_TICKS, Row, Tone, card, copy_popup, cut, draw_rows, item_popup, item_row, lit,
    marks, slot_popup,
};
use crate::screens::popup::Popup;
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINTER, RULE, TEXT, dim, put, put_paragraph, reveal, solid,
};
use crate::ui::cyberdeck::Pen;
use crate::ui::preview::half;
use crossterm::event::KeyCode;
use game::{ItemKind, Slot};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::cell::Cell;

/// What the viewer's corner says an item can do.
const CAPTION: &str = "space inspect";
/// The box that stands in for a drawing not made yet, in columns and rows.
const MISSING_BOX: (u16, u16) = (21, 5);

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

/// Which half of the top holds the cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Rack,
    Pack,
}

/// Where the cursor is, kept between visits to the tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KitState {
    pub pane: Pane,
    /// Index into `Slot::ALL`.
    pub tile: usize,
    /// The pack's row, headers counted.
    pub row: usize,
    /// The kind the pack shows; `None` is all of them.
    pub filter: Option<ItemKind>,
}

/// Rows the card and the drawing take under the rule.
pub(crate) const C_ROWS: u16 = 12;
/// Columns of the item card.
pub(crate) const CARD_W: u16 = 41;

/// The body's parts: the rack and the pack side by side over a rule, and under it the
/// card and the drawing.
pub(crate) struct Layout {
    pub rack: Rect,
    pub divider: u16,
    pub pack: Rect,
    pub rule: u16,
    pub card: Rect,
    pub view: Rect,
}

pub(crate) fn layout(body: Rect) -> Layout {
    let top = body.height.saturating_sub(C_ROWS + 1);
    let a_w = body.width.saturating_sub(1) / 2;
    let divider = body.x + a_w;
    let rule = body.y + top;
    let view_x = body.x + 45;
    Layout {
        rack: Rect::new(body.x, body.y, a_w, top),
        divider,
        pack: Rect::new(
            divider + 1,
            body.y,
            body.right().saturating_sub(divider + 1),
            top,
        ),
        rule,
        card: Rect::new(body.x + 2, rule + 1, CARD_W, C_ROWS),
        view: Rect::new(
            view_x,
            rule + 1,
            body.right().saturating_sub(1 + view_x),
            C_ROWS,
        ),
    }
}

/// The `i`th slot's tile in the rack: three across, three rows of three.
pub(crate) fn tile_rect(rack: Rect, i: usize) -> Rect {
    let w = rack.width / 3;
    Rect::new(
        rack.x + (i % 3) as u16 * w,
        rack.y + (i / 3) as u16 * 3,
        w,
        3,
    )
}

pub struct Kit {
    pub state: KitState,
    /// Tick of the last selection or pane change; drives the card's typewriter.
    pub changed: u64,
    seg_from: Pane,
    seg: Tween,
    /// Columns a pack row's text had at the last draw, after the cursor's `▸ `.
    room: Cell<usize>,
    /// The body at the last draw, to anchor popups.
    body: Cell<Rect>,
    /// The pack's first visible row at the last draw.
    first: Cell<usize>,
}

impl Kit {
    pub fn new(tick: u64) -> Kit {
        Kit {
            state: KitState::default(),
            changed: tick,
            seg_from: Pane::Rack,
            seg: Tween::new(tick, 6, Easing::OutCubic),
            room: Cell::new(usize::MAX),
            body: Cell::new(Rect::default()),
            first: Cell::new(0),
        }
    }

    /// Reopens where the tab was left (`st.deck_kit`), the cursor kept on something
    /// that still exists. The card types out afresh; the focus segment sits still under
    /// the focused pane.
    pub fn restore(st: &AppState) -> Kit {
        let mut kit = Kit::new(st.tick);
        kit.state = st.deck_kit;
        kit.seg_from = kit.state.pane;
        kit.reselect(st);
        kit
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

    /// What the cursor is on: the tile's slot, or the pack row's item.
    pub(crate) fn selected(&self, st: &AppState) -> Option<Detail> {
        match self.state.pane {
            Pane::Rack => Slot::ALL.get(self.state.tile).map(|s| Detail::Slot(*s)),
            Pane::Pack => self.pack_rows(st).get(self.state.row)?.detail.clone(),
        }
    }

    fn set_pane(&mut self, pane: Pane, tick: u64) {
        self.seg_from = self.state.pane;
        self.seg = Tween::new(tick, 6, Easing::OutCubic);
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

    /// Handles a key; true when it was the kit's.
    pub(crate) fn key(&mut self, st: &AppState, code: KeyCode, tick: u64) -> bool {
        match code {
            KeyCode::Char('[') => self.step_filter(st, -1, tick),
            KeyCode::Char(']') => self.step_filter(st, 1, tick),
            KeyCode::Up
            | KeyCode::Char('k')
            | KeyCode::Down
            | KeyCode::Char('j')
            | KeyCode::Left
            | KeyCode::Char('h')
            | KeyCode::Right
            | KeyCode::Char('l') => {
                let up = matches!(code, KeyCode::Up | KeyCode::Char('k'));
                let down = matches!(code, KeyCode::Down | KeyCode::Char('j'));
                let left = matches!(code, KeyCode::Left | KeyCode::Char('h'));
                match self.state.pane {
                    Pane::Rack => {
                        let t = self.state.tile;
                        if up && t >= 3 {
                            self.state.tile -= 3;
                        } else if down && t + 3 < Slot::ALL.len() {
                            self.state.tile += 3;
                        } else if left && !t.is_multiple_of(3) {
                            self.state.tile -= 1;
                        } else if !(up || down || left) {
                            if t % 3 < 2 && t + 1 < Slot::ALL.len() {
                                self.state.tile += 1;
                            } else {
                                self.set_pane(Pane::Pack, tick);
                            }
                        } else {
                            return true;
                        }
                        self.changed = tick;
                    }
                    Pane::Pack => {
                        if up {
                            self.move_row(st, -1, tick);
                        } else if down {
                            self.move_row(st, 1, tick);
                        } else if left {
                            self.set_pane(Pane::Rack, tick);
                        }
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Keeps the cursor on something that exists after the gear or the pack changed.
    pub(crate) fn reselect(&mut self, st: &AppState) {
        self.state.tile = self.state.tile.min(Slot::ALL.len() - 1);
        let rows = self.pack_rows(st);
        if !rows.get(self.state.row).is_some_and(|r| r.detail.is_some()) {
            self.state.row = rows.iter().position(|r| r.detail.is_some()).unwrap_or(0);
        }
    }

    /// The popup for the selection: a tile's slot, or a pack row's item or copy.
    pub(crate) fn interact(&self, st: &mut AppState) -> Option<Popup> {
        let detail = self.selected(st)?;
        let l = layout(self.body.get());
        match detail {
            Detail::Slot(slot) => {
                let tile = tile_rect(l.rack, self.state.tile);
                slot_popup(st, slot, (tile.right() + 1, tile.y + 1))
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
            Pane::Rack => {
                "←↑↓→ rack, then over to the pack · [ ] filter · space act · Q/E tabs · tab close"
            }
            Pane::Pack => {
                "←→ gear/pack · ↑↓ select · [ ] filter · space act · Q/E tabs · tab close"
            }
        }
    }

    /// Draws the tab into `body`: the rack and the pack over the rule, and under it the
    /// selection's card beside its drawing. Keeps the body and the pack's first row for
    /// the popups.
    pub(crate) fn draw(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        let tick = st.tick;
        let l = layout(body);
        self.body.set(body);
        // Rows draw after the cursor's two columns, one in from the pane's edge.
        self.room.set(l.pack.width.saturating_sub(3) as usize);
        for i in 0..Slot::ALL.len() {
            self.draw_tile(st, l.rack, i, buf);
        }
        self.draw_pack(st, l.pack, buf);
        self.draw_rule(st, body, &l, buf);

        let selected = self.selected(st);
        let lines = match &selected {
            Some(d) => card(st, d),
            None => vec![dim("nothing here")],
        };
        let p = tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
        put_paragraph(buf, l.card, reveal(&lines, p, (tick / 4).is_multiple_of(2)));
        draw_viewer(st, selected.as_ref(), l.view, buf);
    }

    /// One slot's tile: its label set in the top edge, the worn item's name inside and
    /// its marks set in the bottom edge. The selected tile's edge is cyan while the rack
    /// has the focus, dim while it does not.
    fn draw_tile(&self, st: &AppState, rack: Rect, i: usize, buf: &mut Buffer) {
        let Some(&slot) = Slot::ALL.get(i) else {
            return;
        };
        let t = tile_rect(rack, i);
        let (x, y, w, right) = (t.x, t.y, t.width, t.right());
        let inner = right.saturating_sub(1);
        let focused = self.state.pane == Pane::Rack;
        let sel = i == self.state.tile;
        let on = sel && focused;
        let edge = Style::default().fg(match (sel, focused) {
            (false, _) => RULE,
            (true, true) => CYAN,
            (true, false) => DIM,
        });

        let label = slot.label().to_uppercase();
        let run = "─".repeat((w as usize).saturating_sub(label.chars().count() + 4));
        put(buf, x, y, &format!("┌ {label} {run}┐"), edge, right);
        let label_style = if on {
            lit_bold(st.tick)
        } else {
            Style::default().fg(DIM)
        };
        put(buf, x + 2, y, &label, label_style, inner);

        put(buf, x, y + 1, "│", edge, right);
        put(buf, inner, y + 1, "│", edge, right);
        let item = st
            .character
            .equipped(slot)
            .and_then(|id| st.content.item(id));
        let mut style = Style::default().fg(match item {
            None => FAINTER,
            Some(_) if on => lit(st.tick),
            Some(_) => TEXT,
        });
        if on {
            for cx in x + 1..inner {
                buf[(cx, y + 1)].set_bg(BAR_BG);
            }
            style = style.bg(BAR_BG).add_modifier(Modifier::BOLD);
        }
        let name = item.map_or("— empty —", |i| i.name.as_str());
        let room = w.saturating_sub(2) as usize;
        put(buf, x + 1, y + 1, &cut(name, room), style, inner);

        let run = "─".repeat(room);
        put(buf, x, y + 2, &format!("└{run}┘"), edge, right);
        let Some(item) = item else {
            return;
        };
        let marks = marks(st, item, st.character.worn_copy(slot), true, on);
        if marks.is_empty() {
            return;
        }
        let mut cx = x + 2;
        for (text, style) in std::iter::once((" ", edge))
            .chain(marks.iter().map(|m| (m.content.as_ref(), m.style)))
            .chain(std::iter::once((" ", edge)))
        {
            put(buf, cx, y + 2, text, style, inner);
            cx = cx.saturating_add(text.chars().count() as u16);
        }
    }

    /// The pack: the filter's chips on its first row, ALL then a kind's glyph each, the
    /// active one bracketed and named; under them the list.
    fn draw_pack(&self, st: &AppState, pack: Rect, buf: &mut Buffer) {
        let focused = self.state.pane == Pane::Pack;
        let mut x = pack.x + 1;
        for chip in std::iter::once(None).chain(ItemKind::ALL.into_iter().map(Some)) {
            let on = chip == self.state.filter;
            let text = match (chip, on) {
                (None, true) => "▌ALL▐".to_string(),
                (None, false) => " ALL ".to_string(),
                (Some(kind), true) => {
                    format!("▌{} {}▐", kind_icon(kind), kind.label().to_uppercase())
                }
                (Some(kind), false) => format!(" {} ", kind_icon(kind)),
            };
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

    /// The divider between the panes and the rule under them, with the cyan segment
    /// under the focused pane sliding as the tab's does, and the weight and worth
    /// of everything carried, worn included, set into the rule's right end.
    fn draw_rule(&self, st: &AppState, body: Rect, l: &Layout, buf: &mut Buffer) {
        for cy in body.y..l.rule {
            buf[(l.divider, cy)].set_char('║').set_fg(RULE);
        }
        for cx in body.x..body.right() {
            buf[(cx, l.rule)].set_char('═').set_fg(RULE);
        }
        buf[(l.divider, l.rule)].set_char('╩').set_fg(RULE);

        let span = |pane: Pane| match pane {
            Pane::Rack => (body.x, l.divider),
            Pane::Pack => (l.divider + 1, body.right()),
        };
        let p = self.seg.progress(st.tick);
        let between = |a: u16, b: u16| (a as f32 + (b as f32 - a as f32) * p).round() as u16;
        let ((a0, a1), (b0, b1)) = (span(self.seg_from), span(self.state.pane));
        for cx in between(a0, b0)..between(a1, b1) {
            buf[(cx, l.rule)].set_char('━').set_fg(CYAN);
        }
        buf[(l.divider, l.rule)].set_char('╩').set_fg(RULE);

        let total = format!(
            " {:.1} kg · ¤ {} ",
            st.character.weight(&st.content),
            st.character.value(&st.content)
        );
        let end = l.pack.right().saturating_sub(2);
        let x = end.saturating_sub(total.chars().count() as u16);
        put(buf, x, l.rule, &total, Style::default().fg(DIM), end);
    }
}

/// The viewer: a solid window with corner ticks and the selection's drawing at half
/// size, centred and clipped to it, or the box that says it has none yet. An empty
/// slot, or nothing selected, leaves only the ticks.
fn draw_viewer(st: &AppState, selected: Option<&Detail>, view: Rect, buf: &mut Buffer) {
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

    let shown = match selected {
        Some(Detail::Slot(slot)) => st.character.equipped(*slot).map(|id| {
            let copy = st.character.worn_copy(*slot).map(|k| k.id);
            (id.clone(), true, copy)
        }),
        Some(Detail::Item(id)) => Some((id.clone(), false, None)),
        Some(Detail::Copy(copy)) => st
            .character
            .copy_in_pack(*copy)
            .map(|s| (s.item.clone(), false, Some(*copy))),
        _ => None,
    };
    let Some((item, equipped, copy)) = shown else {
        return;
    };
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
mod tests {
    use super::*;
    use crate::actions::perform;
    use crate::ui::chrome::PANEL_BG;
    use game::{Action, Character, Host, ItemId};
    use ratatui::style::Color;

    /// The deck's body at 100×30.
    const BODY: Rect = Rect::new(1, 5, 98, 22);

    fn state() -> AppState {
        let mut st = AppState::new();
        st.character = Character::debug_runner(&st.content);
        st
    }

    /// The tick at which the card has typed out and the segment has stopped.
    fn settle(k: &Kit, st: &mut AppState) {
        st.tick = k.changed + REVEAL_TICKS + 40;
    }

    fn drawn(k: &Kit, st: &AppState) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        k.draw(st, BODY, &mut buf);
        buf
    }

    /// `n` cells of row `y` from column `x`.
    fn cells(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
    }

    /// Row `y` from column `x` to the buffer's right edge.
    fn from(buf: &Buffer, x: u16, y: u16) -> String {
        cells(buf, x, y, buf.area.right() - x)
    }

    /// Fits the first `item` in the pack into a socket of what `slot` wears.
    fn fit(st: &mut AppState, slot: Slot, socket: usize, item: &str) {
        let piece = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == ItemId::new(item))
            .and_then(|s| s.copy_id())
            .unwrap_or_else(|| panic!("a copy of {item} in the pack"));
        perform(
            st,
            Action::Fit {
                host: Host::Worn(slot),
                socket,
                piece,
            },
        )
        .unwrap();
    }

    fn corners_in(buf: &Buffer, view: Rect, fg: Color) {
        let (r, b) = (view.right() - 1, view.bottom() - 1);
        for (x, y, ch) in [
            (view.x, view.y, "┌"),
            (r, view.y, "┐"),
            (view.x, b, "└"),
            (r, b, "┘"),
        ] {
            assert_eq!(
                (buf[(x, y)].symbol(), buf[(x, y)].fg),
                (ch, fg),
                "({x},{y})"
            );
        }
    }

    fn press(k: &mut Kit, st: &AppState, code: KeyCode, tick: u64) -> bool {
        k.key(st, code, tick)
    }

    fn texts(k: &Kit, st: &AppState) -> Vec<String> {
        k.pack_rows(st).into_iter().map(|r| r.text).collect()
    }

    fn to_row(k: &mut Kit, st: &AppState, name: &str) {
        let i = k
            .pack_rows(st)
            .iter()
            .position(|r| r.text.contains(name))
            .expect("row");
        k.state.pane = Pane::Pack;
        k.state.row = i;
    }

    #[test]
    fn layout_at_the_smallest_and_a_common_terminal() {
        let l = layout(Rect::new(1, 5, 98, 22));
        assert_eq!(l.rack, Rect::new(1, 5, 48, 9));
        assert_eq!(l.divider, 49);
        assert_eq!(l.pack, Rect::new(50, 5, 49, 9));
        assert_eq!(l.rule, 14);
        assert_eq!(l.card, Rect::new(3, 15, 41, 12));
        assert_eq!(l.view, Rect::new(46, 15, 52, 12));
        let l = layout(Rect::new(1, 10, 118, 27));
        assert_eq!(l.rack, Rect::new(1, 10, 58, 14));
        assert_eq!(l.divider, 59);
        assert_eq!(l.pack, Rect::new(60, 10, 59, 14));
        assert_eq!(l.rule, 24);
        assert_eq!(l.card, Rect::new(3, 25, 41, 12));
        assert_eq!(l.view, Rect::new(46, 25, 72, 12));
        let l = layout(Rect::new(1, 5, 99, 22));
        assert_eq!(l.divider, 50);
        assert_eq!(l.pack, Rect::new(51, 5, 49, 9));
        assert_eq!(
            tile_rect(Rect::new(1, 5, 48, 9), 5),
            Rect::new(33, 8, 16, 3)
        );
        assert_eq!(
            tile_rect(Rect::new(1, 10, 58, 14), 4),
            Rect::new(20, 13, 19, 3)
        );
    }

    #[test]
    fn the_rack_moves_by_tiles_and_crosses_to_the_pack() {
        let st = state();
        let mut k = Kit::new(0);
        let at = |k: &mut Kit, c: KeyCode| {
            assert!(k.key(&st, c, 5));
            k.state.tile
        };
        // Seven tiles: two rows of three and the deck alone under the first column.
        assert_eq!(at(&mut k, KeyCode::Right), 1);
        assert_eq!(at(&mut k, KeyCode::Down), 4);
        k.changed = 0;
        assert_eq!(at(&mut k, KeyCode::Down), 4);
        assert_eq!(k.changed, 0, "a stop is not a move");
        assert_eq!(at(&mut k, KeyCode::Left), 3);
        assert_eq!(at(&mut k, KeyCode::Down), 6);
        assert_eq!(at(&mut k, KeyCode::Down), 6);
        assert_eq!(at(&mut k, KeyCode::Left), 6);
        for _ in 0..3 {
            at(&mut k, KeyCode::Up);
        }
        assert_eq!(k.state.tile, 0);
        at(&mut k, KeyCode::Char('l'));
        assert_eq!(at(&mut k, KeyCode::Char('l')), 2);
        assert_eq!(k.changed, 5);
        assert_eq!(at(&mut k, KeyCode::Right), 2);
        assert_eq!(k.state.pane, Pane::Pack);
        assert_eq!(at(&mut k, KeyCode::Left), 2);
        assert_eq!(k.state.pane, Pane::Rack);
        assert_eq!(at(&mut k, KeyCode::Char('j')), 5);
    }

    #[test]
    fn the_pack_wraps_and_left_returns_to_the_tile() {
        let st = state();
        let mut k = Kit::new(0);
        k.state.pane = Pane::Pack;
        k.reselect(&st);
        let rows = k.pack_rows(&st);
        let sel: Vec<usize> = (0..rows.len())
            .filter(|&i| rows[i].detail.is_some())
            .collect();
        assert_eq!(k.state.row, sel[0]);
        assert!(press(&mut k, &st, KeyCode::Up, 1));
        assert_eq!(k.state.row, *sel.last().unwrap());
        assert!(press(&mut k, &st, KeyCode::Down, 2));
        assert_eq!(k.state.row, sel[0]);
        assert!(press(&mut k, &st, KeyCode::Left, 3));
        assert_eq!(k.state.pane, Pane::Rack);
    }

    #[test]
    fn the_filter_steps_through_the_kinds_carried() {
        let mut st = state();
        st.character.inventory.clear();
        st.character
            .give_item(&st.content, ItemId::new("stim_patch"), 1);
        st.character
            .give_item(&st.content, ItemId::new("hushcan"), 1);
        let mut k = Kit::new(0);
        press(&mut k, &st, KeyCode::Char(']'), 1);
        assert_eq!(k.state.filter, Some(ItemKind::Consumable));
        press(&mut k, &st, KeyCode::Char(']'), 1);
        assert_eq!(k.state.filter, Some(ItemKind::Hardware));
        assert_eq!(texts(&k, &st), ["≡  1× Hushcan"]);
        press(&mut k, &st, KeyCode::Char(']'), 1);
        assert_eq!(k.state.filter, None);
        assert_eq!(
            texts(&k, &st),
            [
                "── consumable ──",
                "◍  1× Stim patch",
                "── hardware ──",
                "≡  1× Hushcan"
            ]
        );
        press(&mut k, &st, KeyCode::Char('['), 1);
        assert_eq!(k.state.filter, Some(ItemKind::Hardware));
        assert_eq!(k.state.pane, Pane::Rack);
    }

    #[test]
    fn selected_follows_the_pane() {
        let mut st = state();
        let mut k = Kit::new(0);
        k.state.tile = 4;
        assert_eq!(k.selected(&st), Some(Detail::Slot(Slot::Primary)));
        st.character
            .give_item(&st.content, ItemId::new("hushcan"), 1);
        to_row(&mut k, &st, "Hushcan");
        assert!(matches!(k.selected(&st), Some(Detail::Copy(_))));
    }

    #[test]
    fn space_on_a_tile_or_a_row_opens_its_popup() {
        let mut st = state();
        let mut k = Kit::new(0);
        k.state.tile = 4;
        let p = k.interact(&mut st).expect("popup");
        let labels: Vec<_> = p.choices().iter().map(|c| c.label.clone()).collect();
        assert!(labels.iter().any(|l| l == "Unequip"), "{labels:?}");
        assert!(labels.iter().any(|l| l == "Inspect"), "{labels:?}");
        to_row(&mut k, &st, "Stim patch");
        let p = k.interact(&mut st).expect("popup");
        let labels: Vec<_> = p.choices().iter().map(|c| c.label.clone()).collect();
        assert_eq!(labels, ["Inspect", "Use", "Drop one"]);
    }

    #[test]
    fn an_empty_pack_selects_nothing() {
        let mut st = state();
        st.character.inventory.clear();
        let mut k = Kit::new(0);
        k.state.pane = Pane::Pack;
        k.reselect(&st);
        assert!(k.pack_rows(&st).is_empty());
        assert_eq!(k.selected(&st), None);
        assert!(k.interact(&mut st).is_none());
        for c in ['j', 'k', ']', '['] {
            press(&mut k, &st, KeyCode::Char(c), 1);
        }
        press(&mut k, &st, KeyCode::Down, 1);
        press(&mut k, &st, KeyCode::Up, 1);
    }

    #[test]
    fn a_filter_left_empty_stays_until_stepped() {
        let mut st = state();
        let mut k = Kit::new(0);
        k.state.filter = Some(ItemKind::Hardware);
        let content = st.content.clone();
        st.character.inventory.retain(|s| {
            content
                .item(&s.item)
                .is_some_and(|i| i.kind != ItemKind::Hardware)
        });
        k.reselect(&st);
        assert_eq!(k.state.filter, Some(ItemKind::Hardware));
        assert!(k.pack_rows(&st).is_empty());
        // Stepped: on to the next kind the pack holds, his stickers, then all.
        press(&mut k, &st, KeyCode::Char(']'), 1);
        assert_eq!(k.state.filter, Some(ItemKind::Sticker));
        press(&mut k, &st, KeyCode::Char(']'), 1);
        assert_eq!(k.state.filter, None);
    }

    #[test]
    fn a_row_that_leaves_moves_the_cursor_to_a_row() {
        let mut st = state();
        let mut k = Kit::new(0);
        to_row(&mut k, &st, "Pawnshop deck");
        // Held per copy now: it carries stickers.
        let Some(Detail::Copy(copy)) = k.selected(&st) else {
            panic!("no pawnshop deck selected");
        };
        perform(&mut st, Action::EquipCopy(copy)).unwrap();
        k.reselect(&st);
        let rows = k.pack_rows(&st);
        assert!(rows[k.state.row].detail.is_some());
        assert!(k.selected(&st).is_some());
    }

    #[test]
    fn the_rack_draws_tiles_with_marks_in_their_edges() {
        let mut st = state();
        fit(&mut st, Slot::Deck, 0, "coldfin_2");
        let mut k = Kit::new(0);
        k.state.tile = 6;
        settle(&k, &mut st);
        let buf = drawn(&k, &st);
        assert_eq!(cells(&buf, 1, 11, 16), "┌ DECK ────────┐");
        assert_eq!(cells(&buf, 1, 12, 16), "│Rebuilt pawn… │");
        assert_eq!(cells(&buf, 1, 13, 16), "└─ ■■■■■ · ■□ ─┘");
        assert_eq!(buf[(1, 11)].fg, CYAN);
        assert_eq!(buf[(2, 12)].bg, BAR_BG);
        assert_eq!(buf[(4, 13)].fg, Color::Rgb(0x1a, 0x9a, 0xaa));
        assert_eq!(buf[(10, 13)].fg, Color::Rgb(0x00, 0xa5, 0xb8));
        assert_eq!(buf[(12, 13)].fg, Color::Rgb(0x00, 0xe5, 0xff));
        assert_eq!(buf[(13, 13)].fg, Color::Rgb(0x00, 0xa5, 0xb8));
        assert_eq!((buf[(2, 6)].symbol(), buf[(2, 6)].fg), ("—", FAINTER));

        k.state.pane = Pane::Pack;
        k.reselect(&st);
        let buf = drawn(&k, &st);
        assert_eq!(buf[(1, 11)].fg, DIM, "the tile keeps its place, dim");
        assert_ne!(buf[(2, 12)].bg, BAR_BG);
        assert_eq!(buf[(4, 13)].fg, Color::Rgb(0x3f, 0x7c, 0x87));
        assert_eq!(buf[(10, 13)].fg, DIM);
        assert_eq!(buf[(13, 13)].fg, DIM);
    }

    #[test]
    fn the_pack_draws_its_chips_then_its_list() {
        let mut st = state();
        let mut k = Kit::new(0);
        k.state.pane = Pane::Pack;
        k.reselect(&st);
        settle(&k, &mut st);
        let buf = drawn(&k, &st);
        let chips = from(&buf, 51, 5);
        assert!(chips.starts_with("▌ALL▐ /  ▣  ◈  ⌗ "), "{chips:?}");
        assert_eq!(buf[(57, 5)].fg, FAINTER, "no weapon in the pack");
        assert_eq!(buf[(66, 5)].fg, DIM, "a deck in the pack");

        // Back from ALL: his stickers, the last kind, then the hardware.
        assert!(k.key(&st, KeyCode::Char('['), k.changed));
        assert_eq!(k.state.filter, Some(ItemKind::Sticker));
        assert!(k.key(&st, KeyCode::Char('['), k.changed));
        assert_eq!(k.state.filter, Some(ItemKind::Hardware));
        let buf = drawn(&k, &st);
        let chips = from(&buf, 51, 5);
        assert!(chips.starts_with(" ALL "), "{chips:?}");
        assert!(chips.contains("▌≡ HARDWARE▐"), "{chips:?}");
        let first = from(&buf, 50, 6);
        assert!(first.contains("▸ ≡  1× Coldfin vI"), "{first:?}");
        assert_eq!(buf[(51, 6)].bg, BAR_BG);
    }

    #[test]
    fn the_rule_carries_the_focus_and_the_packs_weight() {
        let mut st = state();
        let mut k = Kit::new(0);
        settle(&k, &mut st);
        let buf = drawn(&k, &st);
        assert_eq!(buf[(49, 14)].symbol(), "╩");
        for x in 1..=48 {
            assert_eq!(
                (buf[(x, 14)].symbol(), buf[(x, 14)].fg),
                ("━", CYAN),
                "x {x}"
            );
        }
        assert_eq!((buf[(50, 14)].symbol(), buf[(50, 14)].fg), ("═", RULE));
        let total = format!(
            " {:.1} kg · ¤ {} ",
            st.character.weight(&st.content),
            st.character.value(&st.content)
        );
        let n = total.chars().count() as u16;
        assert_eq!(cells(&buf, 97 - n, 14, n), total, "ends at x 96");

        k.state.tile = 2;
        let start = 100;
        assert!(k.key(&st, KeyCode::Right, start));
        assert_eq!(k.state.pane, Pane::Pack);
        st.tick = start + 6;
        let buf = drawn(&k, &st);
        assert_eq!(buf[(50, 14)].symbol(), "━");
        assert_eq!(buf[(48, 14)].symbol(), "═");
        st.tick = start + 3;
        let buf = drawn(&k, &st);
        let lead = (0..100).find(|&x| buf[(x, 14)].symbol() == "━");
        assert!(lead.is_some_and(|x| x > 1 && x < 50), "{lead:?}");
    }

    #[test]
    fn the_card_types_beside_the_viewer() {
        let mut st = state();
        fit(&mut st, Slot::Primary, 0, "hushcan");
        fit(&mut st, Slot::Primary, 1, "redeye");
        let mut k = Kit::new(0);
        k.state.tile = 4;
        settle(&k, &mut st);
        let buf = drawn(&k, &st);
        assert!(from(&buf, 3, 15).starts_with("RATCHET SMG"));
        let meta = from(&buf, 3, 16);
        assert!(
            meta.starts_with("weapon · primary · 3.9 kg · ¤ 2150"),
            "{meta:?}"
        );
        corners_in(&buf, Rect::new(46, 15, 52, 12), RULE);
        assert_eq!(cells(&buf, 83, 26, 13), "space inspect");
        let bare = [PANEL_BG, Color::Reset];
        let drawing = (50..94).any(|x| {
            (17..25).any(|y| {
                let c = &buf[(x, y)];
                !bare.contains(&c.fg) || !bare.contains(&c.bg)
            })
        });
        assert!(drawing, "the Ratchet is drawn at half size");
    }

    #[test]
    fn an_item_without_a_drawing_shows_the_missing_box() {
        let mut st = state();
        st.character
            .wear(Slot::Eyes, ItemId::new("mirrorshades"), None);
        let mut k = Kit::new(0);
        k.state.tile = 1;
        settle(&k, &mut st);
        let buf = drawn(&k, &st);
        corners_in(&buf, Rect::new(61, 18, 21, 5), FAINTER);
        assert_eq!((buf[(62, 18)].symbol(), buf[(62, 18)].fg), ("╌", FAINTER));
        assert_eq!((buf[(61, 19)].symbol(), buf[(61, 19)].fg), ("╎", FAINTER));
        assert_eq!((buf[(71, 19)].symbol(), buf[(71, 19)].fg), ("▣", DIM));
        assert_eq!(cells(&buf, 63, 21, 17), "MISSING BLUEPRINT");
        assert_eq!(buf[(63, 21)].fg, FAINTER);
    }

    #[test]
    fn an_empty_slot_shows_an_empty_viewer() {
        let mut st = state();
        assert!(st.character.equipped(Slot::Head).is_none());
        let mut k = Kit::new(0);
        settle(&k, &mut st);
        let view = Rect::new(46, 15, 52, 12);
        let empty = |buf: &Buffer| {
            corners_in(buf, view, RULE);
            let inside = (view.x + 1..view.right() - 1)
                .flat_map(|x| (view.y..view.bottom()).map(move |y| (x, y)))
                .chain((view.y + 1..view.bottom() - 1).map(|y| (view.x, y)))
                .chain((view.y + 1..view.bottom() - 1).map(|y| (view.right() - 1, y)));
            for (x, y) in inside {
                assert_eq!(buf[(x, y)].symbol(), " ", "({x},{y})");
            }
        };
        let buf = drawn(&k, &st);
        assert!(from(&buf, 3, 15).starts_with("— EMPTY —"));
        assert!(!from(&buf, 46, 26).contains("space inspect"));
        empty(&buf);

        st.character.inventory.clear();
        k.state.pane = Pane::Pack;
        k.reselect(&st);
        let buf = drawn(&k, &st);
        assert!(from(&buf, 3, 15).starts_with("nothing here"));
        empty(&buf);
    }

    #[test]
    fn nothing_spills_outside_the_body() {
        let mut st = state();
        let area = Rect::new(0, 0, 120, 40);
        let last = *ItemKind::ALL.last().unwrap();
        for body in [
            Rect::new(1, 5, 98, 22),
            Rect::new(1, 5, 99, 23),
            Rect::new(1, 10, 118, 27),
        ] {
            for pane in [Pane::Rack, Pane::Pack] {
                let mut k = Kit::new(0);
                k.state.pane = pane;
                k.state.filter = Some(ItemKind::Hardware);
                k.reselect(&st);
                settle(&k, &mut st);
                let mut buf = Buffer::empty(area);
                for c in &mut buf.content {
                    c.set_char('X');
                }
                k.draw(&st, body, &mut buf);
                for y in area.y..area.bottom() {
                    for x in area.x..area.right() {
                        if !body.contains((x, y).into()) {
                            assert_eq!(buf[(x, y)].symbol(), "X", "{body:?} {pane:?} ({x},{y})");
                        }
                    }
                }
            }
            // Every chip row fits the pack, the widest active chip included.
            let pack = layout(body).pack;
            let filters = std::iter::once(None).chain(ItemKind::ALL.into_iter().map(Some));
            for filter in filters {
                let mut k = Kit::new(0);
                k.state.filter = filter;
                let mut buf = Buffer::empty(area);
                k.draw(&st, body, &mut buf);
                let row = cells(&buf, pack.x, pack.y, pack.width);
                let active = filter.map_or("▌ALL▐".to_string(), |kind| {
                    format!("▌{} {}▐", kind_icon(kind), kind.label().to_uppercase())
                });
                assert!(row.contains(&active), "{body:?} {row:?}");
                let end = if filter == Some(last) {
                    '▐'
                } else {
                    kind_icon(last)
                };
                assert!(row.trim_end().ends_with(end), "{body:?} {row:?}");
            }
        }
    }
    #[test]
    fn filter_keys_retype_the_card_only_from_the_pack() {
        let st = state();
        let mut k = Kit::new(0);
        press(&mut k, &st, KeyCode::Char(']'), 7);
        assert_eq!(k.changed, 0, "the rack's card stays as it was");
        k.state.pane = Pane::Pack;
        press(&mut k, &st, KeyCode::Char(']'), 9);
        assert_eq!(k.changed, 9);
    }

    #[test]
    fn the_divider_keeps_its_junction_mid_slide() {
        let mut st = state();
        let mut k = Kit::new(0);
        k.set_pane(Pane::Pack, 0);
        st.tick = 3;
        let buf = drawn(&k, &st);
        let l = layout(BODY);
        let seg: Vec<u16> = (BODY.x..BODY.right())
            .filter(|&x| buf[(x, l.rule)].symbol() == "━")
            .collect();
        assert!(seg.first().is_some_and(|&s| s < l.divider), "{seg:?}");
        assert_eq!(buf[(l.divider, l.rule)].symbol(), "╩");
        assert_eq!(buf[(l.divider, l.rule)].fg, RULE);
    }
}
