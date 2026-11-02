//! What the sidebar's minimap shows of the track on one map: the walk, the target and
//! the floor it wants, and the bottom row's right side (spec
//! `docs/superpowers/specs/2026-11-02-minimap-design.md`, 2, 3 and 5).

use super::listings::{marker_of, target_tile};
use super::route::NoLeg;
use super::{Target, distance, floor_wanted, leg_now, next_now, target_now};
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM};
use citygen::{Place, Pos};
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// The track as the minimap of one map draws it; nothing at all when untracked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapTrack<'a> {
    /// The leg's tiles when the leg is on this map.
    pub walk: &'a [Pos],
    /// The target's tile on this map and its marker.
    pub target: Option<(Pos, char, Color)>,
    /// The floor of this building the track wants you on.
    pub wanted: Option<i32>,
    /// The bottom row's right side.
    pub right: Vec<Span<'static>>,
}

/// One of the eight arrows from `from` towards `to`, north up; none on the spot.
pub fn bearing(from: Pos, to: Pos) -> Option<char> {
    let (dx, dy) = ((to.x - from.x) as f32, (to.y - from.y) as f32);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    // Clockwise from east, a sector of 45° round each arrow.
    let a = dy.atan2(dx).to_degrees().rem_euclid(360.0);
    let i = ((a + 22.5) / 45.0) as usize % 8;
    Some(['→', '↘', '↓', '↙', '←', '↖', '↑', '↗'][i])
}

/// Where a located target stands on `place`.
fn target_on(st: &AppState, place: Place, t: Target) -> Option<Pos> {
    match (place, t) {
        (Place::Stratum(k), _) => target_tile(st, k, t),
        (_, Target::Spot { place: p, pos, .. }) => (p == place).then_some(pos),
        _ => None,
    }
}

/// What the minimap of `place` shows of the track.
pub fn map_track(st: &AppState, place: Place) -> MapTrack<'_> {
    let Some(track) = st.track.as_ref() else {
        return MapTrack::default();
    };
    let leg = leg_now(st);
    let walk = match leg {
        Some(Ok(l)) if l.place == place => l.path.as_slice(),
        _ => &[],
    };
    let target = target_now(st)
        .and_then(|t| target_on(st, place, t))
        .map(|p| {
            let (glyph, colour) = marker_of(st, &track.source);
            (p, glyph, colour)
        });
    let wanted = match place {
        Place::Floor { building, .. } => floor_wanted(st, building),
        _ => None,
    };
    let you = st
        .player
        .as_ref()
        .filter(|p| p.place == place)
        .map(|p| p.pos);
    let dim = |t: String| Span::styled(t, Style::default().fg(DIM));
    let mark = || Span::styled("⌖", Style::default().fg(CYAN));
    let right = match leg {
        Some(Err(NoLeg::NoWay)) => vec![mark(), dim(" no way".to_string())],
        Some(Err(NoLeg::JackedIn)) => vec![mark(), dim(" jacked in".to_string())],
        _ => {
            let m = distance(st).unwrap_or(0);
            match target {
                Some((p, glyph, colour)) => {
                    let arrow = you
                        .and_then(|y| bearing(y, p))
                        .map_or(String::new(), |a| format!(" {a}"));
                    vec![
                        Span::styled(glyph.to_string(), Style::default().fg(colour)),
                        dim(format!(" {m} m{arrow}")),
                    ]
                }
                None => {
                    let next = next_now(st).map_or(format!("{m} m"), str::to_string);
                    vec![mark(), dim(format!(" {next}"))]
                }
            }
        }
    };
    MapTrack {
        walk,
        target,
        wanted,
        right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::shots::{tracked_city, tracked_from_the_flat};
    use citygen::StratumKind;

    fn text(t: &MapTrack) -> String {
        t.right.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn bearing_points_eight_ways() {
        let o = Pos::new(0, 0);
        assert_eq!(bearing(o, Pos::new(5, 0)), Some('→'));
        assert_eq!(bearing(o, Pos::new(0, -5)), Some('↑'));
        assert_eq!(bearing(o, Pos::new(0, 5)), Some('↓'));
        assert_eq!(bearing(o, Pos::new(4, 4)), Some('↘'));
        assert_eq!(bearing(o, Pos::new(-4, -5)), Some('↖'));
        assert_eq!(bearing(o, Pos::new(-9, 1)), Some('←'));
        assert_eq!(bearing(o, Pos::new(3, -3)), Some('↗'));
        assert_eq!(bearing(o, o), None);
    }

    #[test]
    fn untracked_shows_nothing() {
        let (mut st, _) = tracked_city();
        st.track = None;
        assert_eq!(
            map_track(&st, Place::Stratum(StratumKind::Street)),
            MapTrack::default()
        );
    }

    #[test]
    fn right_side_cases() {
        let street = Place::Stratum(StratumKind::Street);
        let (st, _) = tracked_city();
        let t = map_track(&st, street);
        let m = crate::track::distance(&st).unwrap();
        let (tile, glyph, _) = t.target.expect("the clinic is on the street");
        assert_eq!(glyph, '+');
        let you = st.player.as_ref().unwrap().pos;
        let arrow = bearing(you, tile).unwrap();
        assert_eq!(text(&t), format!("+ {m} m {arrow}"));
        assert!(!t.walk.is_empty(), "the leg is on the street");
        assert_eq!(t.wanted, None);
        // Not on this map: no walk on the Skyways.
        assert!(
            map_track(&st, Place::Stratum(StratumKind::Skyways))
                .walk
                .is_empty()
        );

        // From the flat the leg rides the lift; the target is not on this floor.
        let (st, _) = tracked_from_the_flat();
        let flat = st.player.as_ref().unwrap().place;
        let t = map_track(&st, flat);
        assert_eq!(t.target, None);
        assert_eq!(t.wanted, Some(0));
        assert!(!t.walk.is_empty(), "the walk to the lift");
        assert_eq!(text(&t), "⌖ lift to L");
    }

    #[test]
    fn no_way_says_so() {
        let (mut st, _) = tracked_city();
        // Step onto a tile walled in all round, so no leg can be made from it.
        let you = st.player.as_ref().unwrap().pos;
        let city = st.city.as_mut().unwrap();
        let cell = you
            .neighbors4()
            .find(|p| city.walkable(StratumKind::Street, *p))
            .expect("a step to take");
        for p in cell.neighbors8() {
            if let Some(t) = city.tile_mut(StratumKind::Street, p) {
                t.terrain = citygen::Terrain::Wall;
                t.feature = None;
            }
        }
        st.player.as_mut().unwrap().pos = cell;
        crate::track::tick(&mut st);
        assert!(matches!(leg_now(&st), Some(Err(NoLeg::NoWay))));
        let t = map_track(&st, Place::Stratum(StratumKind::Street));
        assert_eq!(text(&t), "⌖ no way");
    }
}
