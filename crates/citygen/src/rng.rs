//! Seeded randomness: per-stage RNG derivation and stable per-tile hashes.

use crate::geom::Pos;
use crate::world::StratumKind;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// SplitMix-style mixer; stable across platforms.
pub fn hash64(mut a: u64, b: u64) -> u64 {
    a ^= b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    a = (a ^ (a >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    a = (a ^ (a >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    a ^ (a >> 31)
}

/// An RNG private to one generation stage: changing another stage never reshuffles it.
pub fn stage_rng(seed: u64, stage: u8) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(hash64(seed, 0xC17_u64 + stage as u64 * 0x1000))
}

/// Deterministic 8-bit variant for a tile position on a stratum.
pub fn tile_hash(seed: u64, k: StratumKind, p: Pos) -> u8 {
    let packed = ((p.x as u32 as u64) << 32) | p.y as u32 as u64;
    (hash64(hash64(seed, k.index() as u64), packed) >> 56) as u8
}

pub fn chance(rng: &mut impl Rng, p: f32) -> bool {
    p > 0.0 && rng.random::<f32>() < p
}

/// Pick by integer weight; a zero total falls back to the last item.
pub fn pick_weighted<'a, T>(rng: &mut impl Rng, items: &'a [(T, u32)]) -> &'a T {
    let total: u32 = items.iter().map(|(_, w)| *w).sum();
    let mut roll = rng.random_range(0..total.max(1));
    for (item, w) in items {
        if roll < *w {
            return item;
        }
        roll -= *w;
    }
    &items[items.len() - 1].0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Pos;
    use crate::world::StratumKind;
    use rand::Rng;

    #[test]
    fn rng_is_deterministic_and_stage_isolated() {
        let a: u64 = stage_rng(42, 3).random();
        let b: u64 = stage_rng(42, 3).random();
        let c: u64 = stage_rng(42, 4).random();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(
            tile_hash(1, StratumKind::Street, Pos::new(5, 6)),
            tile_hash(1, StratumKind::Street, Pos::new(5, 6))
        );
        assert_ne!(
            tile_hash(1, StratumKind::Street, Pos::new(5, 6)),
            tile_hash(1, StratumKind::Skyways, Pos::new(5, 6))
        );
        let mut r = stage_rng(1, 1);
        let picked = pick_weighted(&mut r, &[("a", 0), ("b", 5)]);
        assert_eq!(*picked, "b");
        assert!(!chance(&mut r, 0.0));
        assert!(chance(&mut r, 1.0));
    }
}
