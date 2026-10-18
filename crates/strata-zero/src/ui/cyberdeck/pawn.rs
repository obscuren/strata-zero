//! The pawn look: the deck drawn as a bench unit, 84 by 22. This holds where its
//! parts sit.

use super::{DeckView, Part};
use ratatui::layout::Rect;

/// The drawing's width.
pub const W: u16 = 84;
/// The drawing's height.
pub const H: u16 = 22;
/// The row the inventory carts sit on.
pub const INV_Y: u16 = 19;
/// A cart in the inventory is this wide before its slots are counted.
const INV_BASE: u16 = 6;
/// Columns the inventory row may reach.
const INV_END: u16 = 81;

/// The pawn look's parts, relative to the drawing's origin.
pub fn parts(view: &DeckView) -> Vec<(Part, Rect)> {
    let mut out = Vec::new();
    // Bay: each loaded program spans its slots; the first empty slot is a stop too.
    let mut start = 0u16;
    for (k, cart) in view.bay.iter().enumerate() {
        let size = u16::from(cart.size.max(1));
        out.push((
            Part::Slot(k),
            Rect::new(8 + 14 * start, 1, 14 * size - 2, 3),
        ));
        start += size;
    }
    if start < u16::from(view.slots) {
        out.push((
            Part::Slot(view.bay.len()),
            Rect::new(8 + 14 * start, 1, 12, 3),
        ));
    }
    out.push((Part::Plate, Rect::new(4, 5, 15, 1)));
    let mut x = 9u16;
    for (k, rom) in view.roms.iter().enumerate() {
        let w = rom.chars().count() as u16 + 2;
        out.push((Part::Rom(k), Rect::new(x, 14, w, 1)));
        x += w + 1;
    }
    out.push((Part::Failsafe, Rect::new(69, 7, 10, 5)));
    out.extend(inventory(view, INV_Y));
    out
}

/// The inventory row at `y`: carts two apart from x 2, starting at `inv_offset`,
/// stopping once one would end past the edge.
pub fn inventory(view: &DeckView, y: u16) -> Vec<(Part, Rect)> {
    let mut out = Vec::new();
    let mut x = 2u16;
    for (k, cart) in view.inventory.iter().enumerate().skip(view.inv_offset) {
        let w = INV_BASE + 6 * u16::from(cart.size.max(1));
        if x + w > INV_END {
            break;
        }
        out.push((Part::Inv(k), Rect::new(x, y, w, 2)));
        x += w + 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::cyberdeck::tests::{cart, kit_view};

    fn rect(parts: &[(Part, Rect)], p: Part) -> Option<Rect> {
        parts.iter().find(|(q, _)| *q == p).map(|(_, r)| *r)
    }

    #[test]
    fn parts_sit_where_the_spec_draws_them() {
        let p = parts(&kit_view());
        assert_eq!(rect(&p, Part::Slot(0)), Some(Rect::new(8, 1, 12, 3)));
        assert_eq!(rect(&p, Part::Slot(1)), Some(Rect::new(22, 1, 12, 3)));
        assert_eq!(rect(&p, Part::Slot(2)), None);
        assert_eq!(rect(&p, Part::Plate), Some(Rect::new(4, 5, 15, 1)));
        assert_eq!(rect(&p, Part::Rom(0)), Some(Rect::new(9, 14, 9, 1)));
        assert_eq!(rect(&p, Part::Rom(1)), Some(Rect::new(19, 14, 11, 1)));
        assert_eq!(rect(&p, Part::Failsafe), Some(Rect::new(69, 7, 10, 5)));
        assert_eq!(rect(&p, Part::Inv(0)), Some(Rect::new(2, 19, 12, 2)));

        let mut v = kit_view();
        v.bay = vec![cart("BACKDOOR", 2)];
        let p = parts(&v);
        assert_eq!(rect(&p, Part::Slot(0)), Some(Rect::new(8, 1, 26, 3)));
        assert_eq!(rect(&p, Part::Slot(1)), None);

        let mut v = kit_view();
        v.bay = vec![cart("CRACK", 1)];
        assert_eq!(
            rect(&parts(&v), Part::Slot(1)),
            Some(Rect::new(22, 1, 12, 3))
        );

        let mut v = kit_view();
        v.roms.clear();
        assert!(!parts(&v).iter().any(|(q, _)| matches!(q, Part::Rom(_))));

        let mut v = kit_view();
        v.slots = 4;
        v.bay = vec![cart("CRACK", 1)];
        let empties: Vec<_> = parts(&v)
            .into_iter()
            .filter_map(|(q, _)| match q {
                Part::Slot(k) if k >= v.bay.len() => Some(k),
                _ => None,
            })
            .collect();
        assert_eq!(empties, vec![1]);
    }

    #[test]
    fn inventory_carts_lie_two_apart() {
        let mut v = kit_view();
        v.inventory = vec![cart("SPOOF", 1), cart("BACKDOOR", 2), cart("MASK", 1)];
        let p = parts(&v);
        let at = |k| rect(&p, Part::Inv(k)).map(|r| (r.x, r.width));
        assert_eq!(at(0), Some((2, 12)));
        assert_eq!(at(1), Some((16, 18)));
        assert_eq!(at(2), Some((36, 12)));

        v.inv_offset = 1;
        let p = parts(&v);
        assert_eq!(rect(&p, Part::Inv(0)), None);
        assert_eq!(rect(&p, Part::Inv(1)).map(|r| r.x), Some(2));
    }
}
