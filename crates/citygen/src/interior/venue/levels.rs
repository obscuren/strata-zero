//! The levels stage: the type's roles placed onto the frame's floors.

use super::{LevelDef, LevelLayout, VenueLayout, VenueType};
use crate::geom::Size;
use rand_chacha::ChaCha8Rng;

/// The levels stage's rng, from the layout seed.
pub(super) const STAGE: u64 = 0x1E7E1;

/// The roles on the floors of a frame whose top level is `top`, lowest first: the below
/// roles from -1 down, the ground at 0, the top roles on the highest floors above 0 (the
/// topmost of them when there are too few floors) and the middle role on the floors
/// between. Without a middle role those floors have no level.
pub(super) fn roles(t: &VenueType, top: i32) -> Vec<(i32, &LevelDef)> {
    let l = &t.levels;
    let mut v: Vec<(i32, &LevelDef)> = l
        .below
        .iter()
        .enumerate()
        .rev()
        .map(|(i, d)| (-1 - i as i32, d))
        .collect();
    v.push((0, &l.ground));
    let top = top.max(0);
    let tops = l.top.len().min(top as usize);
    let first_top = top - tops as i32 + 1;
    if let Some(m) = &l.middle {
        v.extend((1..first_top).map(|n| (n, m)));
    }
    v.extend(
        l.top[l.top.len() - tops..]
            .iter()
            .enumerate()
            .map(|(i, d)| (first_top + i as i32, d)),
    );
    v
}

/// Places the type's roles onto the frame's floors. A level at or above -1 is the
/// frame's size; one below it is its role's `scale` of the frame, growing east and
/// south. Each keeps its role's spine.
pub(super) fn place(
    t: &VenueType,
    layout: &mut VenueLayout,
    _rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    let frame = layout.frame.size;
    layout.levels = roles(t, layout.frame.storeys.1)
        .into_iter()
        .map(|(level, def)| LevelLayout {
            level,
            role: def.name.clone(),
            size: if level < -1 {
                Size {
                    w: frame.w * u32::from(def.scale.0) / 100,
                    h: frame.h * u32::from(def.scale.1) / 100,
                }
            } else {
                frame
            },
            spine: def.spine,
            reserved: Vec::new(),
        })
        .collect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::geom::Size;
    use crate::interior::venue::testutil::test_type;
    use crate::interior::venue::{VenueFrame, lay_out};

    #[test]
    fn roles_fill_the_floors() {
        let t = test_type();
        let own = VenueFrame::own(&t);
        let l = lay_out(&t, own.clone(), 1).unwrap();
        let levels: Vec<i32> = l.levels.iter().map(|x| x.level).collect();
        let below = t.levels.below.len() as i32;
        assert_eq!(levels, (-below..=own.storeys.1).collect::<Vec<_>>());
        assert_eq!(l.level(0).unwrap().role, t.levels.ground.name);
        assert_eq!(l.level(-1).unwrap().role, t.levels.below[0].name);
        assert_eq!(
            l.level(own.storeys.1).unwrap().role,
            t.levels.top.last().unwrap().name
        );
        assert_eq!(
            l.level(1).unwrap().role,
            t.levels.middle.as_ref().unwrap().name
        );
        // Too short for the middle: the top role keeps the top floor; at G only, no level above.
        let short = lay_out(
            &t,
            VenueFrame {
                storeys: (-1, 1),
                ..own.clone()
            },
            1,
        )
        .unwrap();
        assert_eq!(
            short.level(1).unwrap().role,
            t.levels.top.last().unwrap().name
        );
        let flat = lay_out(
            &t,
            VenueFrame {
                storeys: (-1, 0),
                ..own
            },
            1,
        )
        .unwrap();
        assert_eq!(flat.levels.last().unwrap().level, 0);
    }

    #[test]
    fn only_levels_below_minus_one_grow() {
        let t = test_type();
        let l = lay_out(&t, VenueFrame::own(&t), 1).unwrap();
        for x in &l.levels {
            let def = t.role(&x.role).unwrap();
            let want = if x.level < -1 {
                Size {
                    w: l.frame.size.w * def.scale.0 as u32 / 100,
                    h: l.frame.size.h * def.scale.1 as u32 / 100,
                }
            } else {
                l.frame.size
            };
            assert_eq!(x.size, want, "level {}", x.level);
        }
        assert!(l.level(-2).unwrap().size.w > l.frame.size.w);
    }

    #[test]
    fn the_same_seed_lays_the_same_levels() {
        let t = test_type();
        assert_eq!(
            lay_out(&t, VenueFrame::own(&t), 9),
            lay_out(&t, VenueFrame::own(&t), 9)
        );
    }
}
