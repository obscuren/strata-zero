//! Marks: where a scene's beat or an effect points, resolved when it runs (scenes spec
//! §1, "Marks"). All `Home*` marks lie on the player's flat; without a home they are
//! nothing. A `Chair` lies in a cast role's work building, on whichever floor has it.

use super::{Mark, visitors};
use crate::app::AppState;
use crate::interiors;
use crate::render::map::{MapRef, TileMap};
use citygen::{Feature, Place, Pos, Rect};
use sim::Work;

/// The `(Place, Pos)` a mark names now, or None. `walker` is the actor a `Beside`
/// measures from (a visitor's role), else the player.
pub fn resolve(st: &AppState, mark: &Mark, walker: Option<&str>) -> Option<(Place, Pos)> {
    match mark {
        Mark::You => st.player.as_ref().map(|p| (p.place, p.pos)),
        Mark::Role(role) => st
            .missions
            .visitors
            .iter()
            .find(|v| &v.role == role)
            .map(|v| (v.place, v.pos)),
        Mark::Beside(inner) => beside(st, inner, walker),
        Mark::Chair(role) => chair(st, role),
        _ => home_mark(st, mark),
    }
}

/// The operating chair in the role's work building, on the work floor it stands on.
fn chair(st: &AppState, role: &str) -> Option<(Place, Pos)> {
    let id = super::person_of(st, role)?;
    let Work::Building(b) = st.world.as_ref()?.people().get(id.0 as usize)?.work else {
        return None;
    };
    let plan = interiors::plan_now(st, b)?;
    plan.work_floors().into_iter().find_map(|level| {
        let pos = interiors::floor_now(st, &plan, level)?.chair()?;
        Some((Place::Floor { building: b, level }, pos))
    })
}

/// The flat's rect grown by one, its walls included, and the floor it is on.
pub fn flat_rect(st: &AppState) -> Option<(Place, Rect)> {
    let rig = st.home.as_ref()?;
    Some((rig.place(), rig.home.rect.grow(1)))
}

/// The marks that lie on the flat.
fn home_mark(st: &AppState, mark: &Mark) -> Option<(Place, Pos)> {
    let rig = st.home.as_ref()?;
    let home = rig.home;
    let place = rig.place();
    let map = MapRef::for_place(
        st.city.as_ref()?,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    )?;
    // The step from the door into the flat.
    let inward = home
        .door
        .neighbors4()
        .find(|n| home.rect.contains(*n))
        .map(|n| (n.x - home.door.x, n.y - home.door.y));
    let pos = match mark {
        Mark::HomeDoor => Some(home.door),
        Mark::HomeOutside => {
            let free = |p: &Pos| !home.rect.contains(*p) && map.walkable(*p);
            inward
                .map(|(dx, dy)| home.door.offset(-dx, -dy))
                .filter(free)
                .or_else(|| home.door.neighbors4().find(free))
        }
        Mark::HomeInside(n) => {
            let (dx, dy) = inward?;
            (1..=*n as i32)
                .map(|k| home.door.offset(dx * k, dy * k))
                .take_while(|p| home.rect.contains(*p) && map.walkable(*p))
                .last()
        }
        Mark::HomeRig => nearest_door(
            home.rig
                .neighbors4()
                .filter(|p| home.rect.contains(*p) && map.walkable(*p)),
            home.door,
        ),
        Mark::HomeBed => Some(home.bed_side),
        Mark::HomeFurniture(f) => nearest_door(
            home.rect.iter().filter(|p| {
                map.tile(*p)
                    .is_some_and(|t| t.feature == Some(Feature::Furniture(*f)))
            }),
            home.door,
        ),
        _ => None,
    }?;
    Some((place, pos))
}

/// The tile nearest the door; ties go to the top, then the left.
fn nearest_door(tiles: impl Iterator<Item = Pos>, door: Pos) -> Option<Pos> {
    tiles.min_by_key(|p| (p.manhattan(door), p.y, p.x))
}

/// The free walkable tile next to `inner`, nearest the walker (or the player).
fn beside(st: &AppState, inner: &Mark, walker: Option<&str>) -> Option<(Place, Pos)> {
    let (place, at) = resolve(st, inner, walker)?;
    beside_tile(st, place, at, walker)
}

/// The free walkable tile next to `at` on `place`, nearest the walker (or the player):
/// free is no actor on it. Next door first, then the corners.
pub fn beside_tile(
    st: &AppState,
    place: Place,
    at: Pos,
    walker: Option<&str>,
) -> Option<(Place, Pos)> {
    let map = MapRef::for_place(
        st.city.as_ref()?,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    )?;
    let from = walker
        .and_then(|r| st.missions.visitors.iter().find(|v| v.role == r))
        .map(|v| (v.place, v.pos))
        .or_else(|| st.player.as_ref().map(|p| (p.place, p.pos)))
        .filter(|(pl, _)| *pl == place)
        .map_or(at, |(_, pos)| pos);
    let free = |p: &Pos| {
        map.walkable(*p)
            && visitors::at(st, place, *p).is_none()
            && !st
                .player
                .as_ref()
                .is_some_and(|pl| pl.place == place && pl.pos == *p)
    };
    let nearest = |tiles: &mut dyn Iterator<Item = Pos>| {
        tiles
            .filter(free)
            .min_by_key(|p| (p.manhattan(from), p.y, p.x))
    };
    nearest(&mut at.neighbors4())
        .or_else(|| nearest(&mut at.neighbors8()))
        .map(|p| (place, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::walker::Player;
    use citygen::Furniture;

    /// Seed 8, moved in, the player in the flat's first tile.
    fn flat() -> AppState {
        let mut st = state_home();
        let (place, pos) = resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        st
    }

    #[test]
    fn home_marks_resolve() {
        let st = flat();
        let home = st.home.unwrap().home;
        let place = st.home.unwrap().place();
        let at = |m: Mark| resolve(&st, &m, None).unwrap_or_else(|| panic!("{m:?} resolves"));
        for m in [
            Mark::HomeDoor,
            Mark::HomeOutside,
            Mark::HomeInside(1),
            Mark::HomeInside(2),
            Mark::HomeRig,
            Mark::HomeBed,
            Mark::HomeFurniture(Furniture::Couch),
        ] {
            assert_eq!(at(m).0, place);
        }
        assert_eq!(at(Mark::HomeDoor).1, home.door);
        let map = MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            place,
            0,
        )
        .unwrap();
        let out = at(Mark::HomeOutside).1;
        assert!(map.walkable(out) && !home.rect.contains(out));
        assert_eq!(out.manhattan(home.door), 1);
        let inside = at(Mark::HomeInside(2)).1;
        assert_eq!(inside.manhattan(home.door), 2);
        assert!(home.rect.contains(inside) && map.walkable(inside));
        let rig = at(Mark::HomeRig).1;
        assert!(map.walkable(rig) && rig.manhattan(home.rig) == 1);
        let couch = at(Mark::HomeFurniture(Furniture::Couch)).1;
        assert_eq!(
            map.tile(couch).unwrap().feature,
            Some(Feature::Furniture(Furniture::Couch))
        );
        assert_eq!(at(Mark::HomeBed).1, home.bed_side);
    }

    #[test]
    fn inside_stops_at_the_last_walkable_tile() {
        let st = flat();
        let far = resolve(&st, &Mark::HomeInside(500), None).unwrap().1;
        let home = st.home.unwrap().home;
        assert!(home.rect.contains(far));
        assert!(far.manhattan(home.door) < 500);
    }

    #[test]
    fn beside_you_is_free_and_next_to_you() {
        let st = flat();
        let me = st.player.as_ref().unwrap().pos;
        let (place, pos) = resolve(&st, &Mark::Beside(Box::new(Mark::You)), None).unwrap();
        assert_eq!(place, st.player.as_ref().unwrap().place);
        assert_eq!(pos.chebyshev(me), 1);
    }

    #[test]
    fn beside_skips_a_visitor() {
        let mut st = flat();
        let me = st.player.as_ref().unwrap();
        let (place, me) = (me.place, me.pos);
        let first = resolve(&st, &Mark::Beside(Box::new(Mark::You)), None).unwrap();
        visitors::arrive(&mut st, "ghost", first);
        let second = resolve(&st, &Mark::Beside(Box::new(Mark::You)), None).unwrap();
        assert_ne!(first.1, second.1);
        assert_eq!((second.0, second.1.chebyshev(me)), (place, 1));
        // A visitor's own mark is where they stand.
        assert_eq!(resolve(&st, &Mark::Role("ghost".into()), None), Some(first));
    }

    #[test]
    fn no_home_no_marks() {
        let mut st = flat();
        st.home = None;
        for m in [
            Mark::HomeDoor,
            Mark::HomeOutside,
            Mark::HomeInside(1),
            Mark::HomeRig,
            Mark::HomeBed,
            Mark::HomeFurniture(Furniture::Couch),
        ] {
            assert_eq!(resolve(&st, &m, None), None, "{m:?}");
        }
        assert_eq!(flat_rect(&st), None);
    }

    #[test]
    fn the_chair_mark_finds_the_doc_chair() {
        let (mut st, b, level, chair) = crate::clinic::testutil::inside();
        st.book = crate::missions::tests::doc_book(crate::missions::tests::IDLE, "");
        let doc = Mark::Chair("doc".into());
        assert_eq!(resolve(&st, &doc, None), None, "nobody cast");
        crate::missions::tests::cast_doc(&mut st, b);
        let here = Place::Floor { building: b, level };
        assert_eq!(resolve(&st, &doc, None), Some((here, chair)));
        // Beside it, on her floor.
        let (place, pos) = resolve(&st, &Mark::Beside(Box::new(doc.clone())), None).unwrap();
        assert_eq!((place, pos.chebyshev(chair)), (here, 1));
        // From the street, the floor nobody has walked onto yet.
        let (mut st, same, _, _) = crate::clinic::testutil::outside(crate::clinic::testutil::NIGHT);
        assert_eq!(same, b, "the fixture's clinic");
        st.book = crate::missions::tests::doc_book(crate::missions::tests::IDLE, "");
        crate::missions::tests::cast_doc(&mut st, b);
        assert_eq!(resolve(&st, &doc, None), Some((here, chair)));
        assert!(
            st.interiors.floor(b, level).is_none(),
            "made fresh, not kept"
        );
    }

    #[test]
    fn the_flat_rect_wraps_the_walls() {
        let st = flat();
        let home = st.home.unwrap().home;
        let (place, r) = flat_rect(&st).unwrap();
        assert_eq!(place, st.home.unwrap().place());
        assert!(r.contains(home.door) && r.contains(home.rect.iter().next().unwrap()));
    }
}
