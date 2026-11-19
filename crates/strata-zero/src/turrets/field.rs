//! A turret's field: the camera's cone, on any of eight facings, and the turning that
//! brings a target into it.

use crate::cameras;
use crate::foes;
use crate::render::map::{MapRef, TileMap};
use citygen::{Dir, Pos};

/// Where `d` stands in `Dir::ALL`: N is 0, clockwise to NW at 7.
fn index(d: Dir) -> i32 {
    Dir::ALL.iter().position(|x| *x == d).unwrap_or(0) as i32
}

/// Clockwise steps of an eighth from `a` to `b`.
fn eighths(a: Dir, b: Dir) -> i32 {
    (index(b) - index(a)).rem_euclid(8)
}

/// The nearest of the eight facings to the line from `from` to `to`.
pub fn toward(from: Pos, to: Pos) -> Dir {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx == 0 && dy == 0 {
        return Dir::N;
    }
    if 12 * dy.abs() < 5 * dx.abs() {
        return if dx > 0 { Dir::E } else { Dir::W };
    }
    if 12 * dx.abs() < 5 * dy.abs() {
        return if dy > 0 { Dir::S } else { Dir::N };
    }
    match (dx > 0, dy > 0) {
        (true, true) => Dir::SE,
        (true, false) => Dir::NE,
        (false, true) => Dir::SW,
        (false, false) => Dir::NW,
    }
}

/// One eighth round toward `goal`, the shorter way (clockwise at half round).
pub fn turn_toward(facing: Dir, goal: Dir) -> Dir {
    let d = eighths(facing, goal);
    let step = match d {
        0 => return facing,
        1..=4 => 1,
        _ => 7,
    };
    Dir::ALL[(index(facing) + step).rem_euclid(8) as usize]
}

/// The facing nearest `facing` whose field holds `target`, clockwise first on a tie;
/// straight toward it when no field does.
pub fn aim(at: Pos, facing: Dir, target: Pos) -> Dir {
    Dir::ALL
        .iter()
        .copied()
        .filter(|f| in_field(at, *f, target))
        .min_by_key(|f| {
            let d = eighths(facing, *f);
            // Clockwise first: of two equal turns, the clockwise one sorts lower.
            (d.min(8 - d), if d <= 4 { 0 } else { 1 })
        })
        .unwrap_or_else(|| toward(at, target))
}

/// Whether `p` lies in the field of a turret at `at` facing `facing`.
pub fn in_field(at: Pos, facing: Dir, p: Pos) -> bool {
    cameras::in_cone_at(at, facing, p)
}

/// Whether a turret sees `p`: in its field, a clear line, and near or lit.
pub fn sees(map: &MapRef, at: Pos, facing: Dir, p: Pos) -> bool {
    in_field(at, facing, p)
        && citygen::sight::sees(
            at,
            p,
            cameras::DARK_REACH,
            |q| map.tile(q).is_none_or(foes::blocks_sight),
            |q| foes::floor_light(map, q),
            |_| false,
        )
}

/// The tiles of its field it has a clear line to.
#[allow(dead_code)] // the map draws it, in the next task
pub fn tiles(map: &MapRef, at: Pos, facing: Dir) -> Vec<Pos> {
    cameras::cone_at(at, facing, |q| map.tile(q).is_none_or(|t| t.blocks_sight()))
}

/// The facing as a compass word.
pub fn dir_word(d: Dir) -> &'static str {
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][index(d) as usize]
}

/// The arrowhead that shows the facing.
pub fn glyph(d: Dir) -> char {
    ['▲', '◥', '►', '◢', '▼', '◣', '◄', '◤'][index(d) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_the_cameras_cone_on_all_eight_facings() {
        let at = Pos::new(10, 10);
        assert!(in_field(at, Dir::E, Pos::new(18, 10)));
        assert!(!in_field(at, Dir::E, Pos::new(19, 10)));
        assert!(in_field(at, Dir::E, Pos::new(12, 12)));
        assert!(!in_field(at, Dir::E, Pos::new(12, 13)));
        assert!(!in_field(at, Dir::E, Pos::new(9, 10)), "not behind it");
        // A diagonal is a quarter: the sum's triangle.
        assert!(in_field(at, Dir::SE, Pos::new(18, 10)));
        assert!(in_field(at, Dir::SE, Pos::new(14, 14)));
        assert!(!in_field(at, Dir::SE, Pos::new(15, 15)));
        assert!(!in_field(at, Dir::SE, Pos::new(11, 9)));
    }

    #[test]
    fn toward_takes_the_nearest_of_the_eight() {
        let o = Pos::new(0, 0);
        assert_eq!(toward(o, Pos::new(5, 1)), Dir::E);
        assert_eq!(toward(o, Pos::new(5, 3)), Dir::SE);
        assert_eq!(toward(o, Pos::new(-1, -6)), Dir::N);
        assert_eq!(toward(o, Pos::new(-4, 4)), Dir::SW);
    }

    #[test]
    fn it_turns_one_eighth_the_shorter_way() {
        assert_eq!(turn_toward(Dir::N, Dir::W), Dir::NW);
        assert_eq!(turn_toward(Dir::N, Dir::E), Dir::NE);
        assert_eq!(
            turn_toward(Dir::N, Dir::S),
            Dir::NE,
            "half round: clockwise"
        );
        assert_eq!(turn_toward(Dir::E, Dir::E), Dir::E);
    }

    #[test]
    fn aim_takes_the_nearest_facing_whose_field_holds_it() {
        let at = Pos::new(10, 3);
        // You four tiles west: NW already holds you, one eighth from N.
        assert_eq!(aim(at, Dir::N, Pos::new(6, 3)), Dir::NW);
        // Five and four off: E holds it, SE (toward) does not.
        assert_eq!(aim(at, Dir::SE, Pos::new(15, 7)), Dir::E);
        // Out of every field: straight toward it.
        assert_eq!(aim(at, Dir::N, Pos::new(30, 3)), Dir::E);
    }

    #[test]
    fn the_glyph_and_word_follow_the_facing() {
        let got: String = Dir::ALL.iter().map(|d| glyph(*d)).collect();
        assert_eq!(got, "▲◥►◢▼◣◄◤");
        assert_eq!(dir_word(Dir::SE), "SE");
    }

    #[test]
    fn it_sees_a_lit_tile_far_and_a_dark_one_only_near() {
        let st = crate::guards::testutil::ward();
        let map = crate::foes::map_of(&st, crate::guards::testutil::on(0)).unwrap();
        // The lit west hall: far is seen.
        assert!(sees(&map, Pos::new(2, 3), Dir::E, Pos::new(9, 3)));
        // The dark east hall (x 16..=23): 6 off unseen, 2 off seen.
        assert!(!sees(&map, Pos::new(17, 3), Dir::E, Pos::new(23, 3)));
        assert!(sees(&map, Pos::new(17, 3), Dir::E, Pos::new(19, 3)));
        // Through the wall at x = 15: never.
        assert!(!sees(&map, Pos::new(12, 3), Dir::E, Pos::new(17, 2)));
    }
}
