//! Light from emitting tiles, as a pure function over a grid.

use crate::geom::{Grid, Rect};
use crate::tile::Tile;

/// Adds every emitter's cone of `strength` over `radius` tiles to `base`, clamped to
/// 0..=255. `emission` gives (radius, strength) for a tile that glows.
pub fn compute(
    tiles: &Grid<Tile>,
    base: u8,
    emission: impl Fn(&Tile) -> Option<(i32, f32)>,
) -> Grid<u8> {
    let size = tiles.size();
    let mut light = vec![base as f32; size.area()];
    for (p, t) in tiles.iter() {
        let Some((r, strength)) = emission(t) else {
            continue;
        };
        for q in Rect::new(p.x - r, p.y - r, 2 * r + 1, 2 * r + 1).iter() {
            if !size.contains(q) {
                continue;
            }
            let d = (p.dist2(q) as f32).sqrt();
            if d > r as f32 {
                continue;
            }
            light[size.index(q)] += strength * (1.0 - d / r as f32);
        }
    }
    let mut out = Grid::filled(size, 0u8);
    for (slot, v) in out.as_mut_slice().iter_mut().zip(light) {
        *slot = v.clamp(0.0, 255.0) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Pos, Size};
    use crate::tile::{Feature, Terrain};

    #[test]
    fn a_lamp_lights_its_surroundings_and_fades_with_distance() {
        let mut tiles = Grid::filled(Size { w: 20, h: 10 }, Tile::new(Terrain::Floor));
        tiles[Pos::new(10, 5)].feature = Some(Feature::Lamp);
        let light = compute(&tiles, 40, |t| match t.feature {
            Some(Feature::Lamp) => Some((4, 120.0)),
            _ => None,
        });
        assert_eq!(light[Pos::new(0, 0)], 40);
        assert!(light[Pos::new(10, 5)] > light[Pos::new(12, 5)]);
        assert!(light[Pos::new(12, 5)] > light[Pos::new(14, 5)]);
        assert_eq!(light[Pos::new(15, 5)], 40);
    }
}
