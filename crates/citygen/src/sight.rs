//! Whether a tile in line of sight is seen: near enough to make out in the dark, or lit.
//! The same rule serves the player's eyes and anyone looking back.

use crate::geom::Pos;
use crate::lighting::tuning::LIT_SIGHT;
use crate::pathing::bresenham;

/// A tile already in line of sight from `origin` counts as seen when it lies within
/// `dark_radius`, throws `LIT_SIGHT` or more of its own light, or is itself a fixture.
pub fn lit_or_near(
    origin: Pos,
    p: Pos,
    dark_radius: i32,
    emitted: impl Fn(Pos) -> u8,
    is_emitter: impl Fn(Pos) -> bool,
) -> bool {
    origin.chebyshev(p) <= dark_radius || emitted(p) >= LIT_SIGHT || is_emitter(p)
}

/// Whether someone at `from` sees the tile `to`: a clear line between them, and `to`
/// near or lit. Not symmetric: the dark sees the lamp, the lamp does not see the dark.
pub fn sees(
    from: Pos,
    to: Pos,
    dark_radius: i32,
    blocks: impl Fn(Pos) -> bool,
    emitted: impl Fn(Pos) -> u8,
    is_emitter: impl Fn(Pos) -> bool,
) -> bool {
    if from == to {
        return true;
    }
    let clear = bresenham(from, to)
        .into_iter()
        .filter(|p| *p != from && *p != to)
        .all(|p| !blocks(p));
    clear && lit_or_near(from, to, dark_radius, emitted, is_emitter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_lit_or_a_fixture_each_count_and_dark_far_does_not() {
        let o = Pos::new(0, 0);
        let dark = |_: Pos| 0u8;
        let none = |_: Pos| false;
        assert!(lit_or_near(o, Pos::new(3, 3), 5, dark, none));
        assert!(!lit_or_near(o, Pos::new(9, 9), 5, dark, none));
        assert!(lit_or_near(o, Pos::new(9, 9), 5, |_| LIT_SIGHT, none));
        assert!(!lit_or_near(o, Pos::new(9, 9), 5, |_| LIT_SIGHT - 1, none));
        assert!(lit_or_near(o, Pos::new(9, 9), 5, dark, |p| p == Pos::new(9, 9)));
    }

    #[test]
    fn the_dark_sees_the_lamp_and_the_lamp_does_not_see_the_dark() {
        let lamp = Pos::new(10, 0);
        let shadow = Pos::new(0, 0);
        let emitted = |p: Pos| if p.chebyshev(lamp) <= 2 { 100 } else { 0 };
        let open = |_: Pos| false;
        let none = |_: Pos| false;
        assert!(sees(shadow, lamp, 3, open, emitted, none));
        assert!(!sees(lamp, shadow, 3, open, emitted, none));
        let wall = |p: Pos| p.x == 5;
        assert!(
            !sees(shadow, lamp, 3, wall, emitted, none),
            "a wall between"
        );
        assert!(
            sees(shadow, Pos::new(2, 0), 3, wall, emitted, none),
            "near, this side"
        );
    }
}
