//! Items lying in the city: piles of stacks per stratum and position.

use crate::character::Stack;
use crate::model::ItemId;
use citygen::{Pos, StratumKind};
use std::collections::HashMap;

/// Everything lying on the ground, keyed by stratum and tile.
#[derive(Clone, Debug, Default)]
pub struct Ground {
    piles: HashMap<(StratumKind, Pos), Vec<Stack>>,
}

impl Ground {
    /// Puts `qty` of an item on a tile, merging into a stack of the same item.
    pub fn add(&mut self, k: StratumKind, p: Pos, item: ItemId, qty: u32) {
        if qty == 0 {
            return;
        }
        let pile = self.piles.entry((k, p)).or_default();
        match pile.iter_mut().find(|s| s.item == item) {
            Some(s) => s.qty = s.qty.saturating_add(qty),
            None => pile.push(Stack { item, qty }),
        }
    }

    /// Takes up to `qty` of an item from a tile and returns how many came away.
    /// Empty stacks and piles disappear.
    pub fn take(&mut self, k: StratumKind, p: Pos, item: &ItemId, qty: u32) -> u32 {
        let Some(pile) = self.piles.get_mut(&(k, p)) else {
            return 0;
        };
        let Some(i) = pile.iter().position(|s| &s.item == item) else {
            return 0;
        };
        let n = qty.min(pile[i].qty);
        pile[i].qty -= n;
        if pile[i].qty == 0 {
            pile.remove(i);
        }
        if pile.is_empty() {
            self.piles.remove(&(k, p));
        }
        n
    }

    pub fn at(&self, k: StratumKind, p: Pos) -> &[Stack] {
        self.piles.get(&(k, p)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn clear(&mut self, k: StratumKind, p: Pos) {
        self.piles.remove(&(k, p));
    }

    /// Every pile on one stratum.
    pub fn on(&self, k: StratumKind) -> impl Iterator<Item = (Pos, &[Stack])> {
        self.piles
            .iter()
            .filter(move |((s, _), _)| *s == k)
            .map(|((_, p), v)| (*p, v.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piles_merge_take_and_vanish() {
        let mut g = Ground::default();
        let (k, p) = (StratumKind::Street, Pos::new(3, 4));
        let coil = ItemId::new("copper_coil");
        g.add(k, p, coil.clone(), 2);
        g.add(k, p, coil.clone(), 3);
        g.add(k, p, ItemId::new("dead_phone"), 1);
        assert_eq!(g.at(k, p).len(), 2);
        assert_eq!(
            g.at(k, p)[0],
            Stack {
                item: coil.clone(),
                qty: 5
            }
        );
        assert_eq!(g.take(k, p, &coil, 4), 4);
        assert_eq!(g.take(k, p, &coil, 4), 1, "only one left");
        assert_eq!(g.at(k, p).len(), 1, "empty stacks vanish");
        assert!(g.at(StratumKind::Undercity, p).is_empty());
        assert_eq!(g.on(k).count(), 1);
        assert_eq!(g.take(k, p, &coil, 1), 0, "nothing left to take");
        g.clear(k, p);
        assert!(g.at(k, p).is_empty());
        assert_eq!(g.on(k).count(), 0, "empty piles vanish");
        g.add(k, p, coil.clone(), u32::MAX);
        g.add(k, p, coil.clone(), 5);
        assert_eq!(g.at(k, p)[0].qty, u32::MAX, "saturates");
    }
}
