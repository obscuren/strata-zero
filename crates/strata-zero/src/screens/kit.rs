//! GEAR & INVENTORY: the rack of what you wear, the pack of what you carry, and the
//! item under the cursor, its card beside its drawing at half size.

// The deck takes this view up; until it does only the tests use it.
#![allow(dead_code)]

use crate::anim::{Easing, Tween};
use crate::app::AppState;
use crate::render::glyphs::kind_icon;
use crate::screens::deck::{
    Detail, Row, Tone, copy_popup, item_popup, item_row, marks, slot_popup,
};
use crate::screens::popup::Popup;
use crossterm::event::KeyCode;
use game::{ItemKind, Slot};
use ratatui::layout::Rect;
use std::cell::Cell;

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
/// Rows the rack's three tile rows take.
pub(crate) const RACK_ROWS: u16 = 9;

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
        let carried = |f: &Option<ItemKind>| match f {
            None => true,
            Some(kind) => st
                .character
                .inventory
                .iter()
                .any(|s| st.content.item(&s.item).is_some_and(|i| i.kind == *kind)),
        };
        let n = order.len() as i32;
        let mut i = order
            .iter()
            .position(|f| *f == self.state.filter)
            .unwrap_or(0) as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
            if carried(&order[i as usize]) {
                break;
            }
        }
        self.state.filter = order[i as usize];
        self.state.row = self
            .pack_rows(st)
            .iter()
            .position(|r| r.detail.is_some())
            .unwrap_or(0);
        self.changed = tick;
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
                        } else if down && t < 6 {
                            self.state.tile += 3;
                        } else if left && !t.is_multiple_of(3) {
                            self.state.tile -= 1;
                        } else if !(up || down || left) {
                            if t % 3 < 2 {
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
                "←↑↓→ rack, then over to the pack · [ ] filter · space act · tab tabs · esc close"
            }
            Pane::Pack => {
                "←→ gear/pack · ↑↓ select · [ ] filter · space act · tab tabs · esc close"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::perform;
    use game::{Action, Character, ItemId};

    fn state() -> AppState {
        let mut st = AppState::new();
        st.character = Character::debug_runner(&st.content);
        st
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
            tile_rect(Rect::new(1, 5, 48, 9), 8),
            Rect::new(33, 11, 16, 3)
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
        assert_eq!(at(&mut k, KeyCode::Right), 1);
        assert_eq!(at(&mut k, KeyCode::Down), 4);
        assert_eq!(at(&mut k, KeyCode::Down), 7);
        k.changed = 0;
        assert_eq!(at(&mut k, KeyCode::Down), 7);
        assert_eq!(k.changed, 0, "a stop is not a move");
        assert_eq!(at(&mut k, KeyCode::Left), 6);
        assert_eq!(at(&mut k, KeyCode::Left), 6);
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
        k.state.tile = 6;
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
        k.state.tile = 6;
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
        press(&mut k, &st, KeyCode::Char(']'), 1);
        assert_eq!(k.state.filter, None);
    }

    #[test]
    fn a_row_that_leaves_moves_the_cursor_to_a_row() {
        let mut st = state();
        let mut k = Kit::new(0);
        to_row(&mut k, &st, "Pawnshop deck");
        let Some(Detail::Item(id)) = k.selected(&st) else {
            panic!("no pawnshop deck selected");
        };
        perform(&mut st, Action::Equip(id)).unwrap();
        k.reselect(&st);
        let rows = k.pack_rows(&st);
        assert!(rows[k.state.row].detail.is_some());
        assert!(k.selected(&st).is_some());
    }
}
