//! Items lying in the city: piles of stacks per place and position.

use crate::character::{CopyId, Stack};
use crate::model::ItemId;
use citygen::{Place, Pos};
use std::collections::HashMap;

/// Everything lying on the ground, keyed by place (a stratum or a floor) and tile.
#[derive(Clone, Debug, Default)]
pub struct Ground {
    piles: HashMap<(Place, Pos), Vec<Stack>>,
}

impl Ground {
    /// Puts a stack on a tile: one without a copy merges into a stack of the same
    /// item without a copy; a copy lies on its own.
    pub fn put(&mut self, place: Place, p: Pos, stack: Stack) {
        if stack.qty == 0 {
            return;
        }
        let pile = self.piles.entry((place, p)).or_default();
        let plain = stack.copy.is_none();
        match pile
            .iter_mut()
            .find(|s| plain && s.copy.is_none() && s.item == stack.item)
        {
            Some(s) => s.qty = s.qty.saturating_add(stack.qty),
            None => pile.push(stack),
        }
    }

    /// Puts `qty` of an item with no state of its own on a tile.
    pub fn add(&mut self, place: Place, p: Pos, item: ItemId, qty: u32) {
        self.put(place, p, Stack::new(item, qty));
    }

    /// Takes up to `qty` of an item from a tile's stacks without a copy and returns
    /// how many came away. Empty stacks and piles disappear.
    pub fn take(&mut self, place: Place, p: Pos, item: &ItemId, qty: u32) -> u32 {
        let Some(pile) = self.piles.get_mut(&(place, p)) else {
            return 0;
        };
        let Some(i) = pile
            .iter()
            .position(|s| &s.item == item && s.copy.is_none())
        else {
            return 0;
        };
        let n = qty.min(pile[i].qty);
        pile[i].qty -= n;
        if pile[i].qty == 0 {
            pile.remove(i);
        }
        if pile.is_empty() {
            self.piles.remove(&(place, p));
        }
        n
    }

    /// Takes one copy from a tile, as it lies.
    pub fn take_copy(&mut self, place: Place, p: Pos, id: CopyId) -> Option<Stack> {
        let pile = self.piles.get_mut(&(place, p))?;
        let i = pile.iter().position(|s| s.copy_id() == Some(id))?;
        let s = pile.remove(i);
        if pile.is_empty() {
            self.piles.remove(&(place, p));
        }
        Some(s)
    }

    /// Takes every stack from a tile, as they lie.
    pub fn take_all(&mut self, place: Place, p: Pos) -> Vec<Stack> {
        self.piles.remove(&(place, p)).unwrap_or_default()
    }

    pub fn at(&self, place: Place, p: Pos) -> &[Stack] {
        self.piles
            .get(&(place, p))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn clear(&mut self, place: Place, p: Pos) {
        self.piles.remove(&(place, p));
    }

    /// Every pile on one map.
    pub fn on(&self, place: Place) -> impl Iterator<Item = (Pos, &[Stack])> {
        self.piles
            .iter()
            .filter(move |((s, _), _)| *s == place)
            .map(|((_, p), v)| (*p, v.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::ItemCopy;
    use citygen::{BuildingId, StratumKind};

    #[test]
    fn piles_merge_take_and_vanish() {
        let mut g = Ground::default();
        let (k, p) = (Place::Stratum(StratumKind::Street), Pos::new(3, 4));
        let coil = ItemId::new("copper_coil");
        g.add(k, p, coil.clone(), 2);
        g.add(k, p, coil.clone(), 3);
        g.add(k, p, ItemId::new("dead_phone"), 1);
        assert_eq!(g.at(k, p).len(), 2);
        assert_eq!(g.at(k, p)[0], Stack::new(coil.clone(), 5));
        assert_eq!(g.take(k, p, &coil, 4), 4);
        assert_eq!(g.take(k, p, &coil, 4), 1, "only one left");
        assert_eq!(g.at(k, p).len(), 1, "empty stacks vanish");
        assert!(g.at(Place::Stratum(StratumKind::Undercity), p).is_empty());
        assert_eq!(g.on(k).count(), 1);
        assert_eq!(g.take(k, p, &coil, 1), 0, "nothing left to take");
        g.clear(k, p);
        assert!(g.at(k, p).is_empty());
        assert_eq!(g.on(k).count(), 0, "empty piles vanish");
        g.add(k, p, coil.clone(), u32::MAX);
        g.add(k, p, coil.clone(), 5);
        assert_eq!(g.at(k, p)[0].qty, u32::MAX, "saturates");
    }

    #[test]
    fn piles_are_kept_per_place() {
        let mut g = Ground::default();
        let p = Pos::new(3, 4);
        let street = Place::Stratum(StratumKind::Street);
        let lobby = Place::Floor {
            building: BuildingId(2),
            level: 0,
        };
        let upstairs = Place::Floor {
            building: BuildingId(2),
            level: 3,
        };
        g.add(street, p, ItemId::new("copper_coil"), 1);
        g.add(lobby, p, ItemId::new("dead_phone"), 2);
        assert_eq!(g.at(street, p)[0].item, ItemId::new("copper_coil"));
        assert_eq!(g.at(lobby, p)[0].qty, 2);
        assert!(g.at(upstairs, p).is_empty());
        assert_eq!(g.on(lobby).count(), 1);
        assert_eq!(g.on(street).count(), 1);
    }

    #[test]
    fn ground_put_merges_only_plain_stacks() {
        let mut g = Ground::default();
        let (k, p) = (Place::Stratum(StratumKind::Street), Pos::new(3, 4));
        let (deck, crack) = (ItemId::new("rebuilt_deck"), ItemId::new("crack"));
        let copy = |n| Stack {
            item: deck.clone(),
            qty: 1,
            copy: Some(ItemCopy {
                id: CopyId(n),
                fitted: vec![None, None],
                stickers: Vec::new(),
            }),
        };
        g.put(k, p, copy(0));
        g.put(k, p, copy(1));
        assert_eq!(g.at(k, p), &[copy(0), copy(1)], "copies never merge");
        g.add(k, p, crack.clone(), 1);
        g.add(k, p, crack.clone(), 1);
        assert_eq!(g.at(k, p).len(), 3);
        assert_eq!(g.at(k, p)[2], Stack::new(crack.clone(), 2));
        assert_eq!(g.take(k, p, &deck, 1), 0, "copies only come by id");
        g.put(k, p, Stack::new(deck.clone(), 1));
        assert_eq!(g.at(k, p).len(), 4, "a plain stack lies beside the copies");
        assert_eq!(g.take(k, p, &deck, 5), 1, "only the plain one");
        assert_eq!(g.take_copy(k, p, CopyId(1)), Some(copy(1)));
        assert_eq!(g.take_copy(k, p, CopyId(1)), None);
        assert_eq!(g.take_all(k, p), vec![copy(0), Stack::new(crack, 2)]);
        assert!(g.at(k, p).is_empty());
        assert_eq!(g.on(k).count(), 0, "empty piles vanish");
        assert!(g.take_all(k, p).is_empty());
        g.put(k, p, copy(2));
        assert_eq!(g.take_copy(k, p, CopyId(2)), Some(copy(2)));
        assert_eq!(g.on(k).count(), 0, "the last copy taken empties the pile");
    }
}
