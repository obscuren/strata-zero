//! The player's home: a whole flat high in the city's first atrium megablock.

use super::{FloorPlan, RoomKind, generate_floor};
use crate::geom::{Pos, Rect};
use crate::ids::BuildingId;
use crate::tile::{Feature, Furniture};
use crate::world::City;

/// Where the player lives: the flat, its front door, the rig they jack in from
/// and the floor beside the bed they wake on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Home {
    pub building: BuildingId,
    pub level: i32,
    pub number: u16,
    /// The flat's interior.
    pub rect: Rect,
    /// The front door onto the walkway.
    pub door: Pos,
    /// The terminal on the bedroom desk.
    pub rig: Pos,
    /// A floor tile beside the bed.
    pub bed_side: Pos,
}

/// The player's home, if the city has a megablock big enough for one.
pub fn home(city: &City) -> Option<Home> {
    let plan = city
        .buildings
        .iter()
        .filter_map(|b| FloorPlan::new(city, b.id))
        .find(|p| p.home_level.is_some())?;
    let level = plan.home_level?;
    let f = generate_floor(city, &plan, level);
    let room = f.rooms.iter().find(|r| r.kind == RoomKind::Home)?;
    let rect = room.rect;
    let rig = rect
        .iter()
        .find(|p| f.tiles[*p].feature == Some(Feature::Terminal))?;
    let bed_side = rect.iter().find(|p| {
        f.tiles[*p].is_walkable()
            && p.neighbors4().any(|n| {
                rect.contains(n) && f.tiles[n].feature == Some(Feature::Furniture(Furniture::Bed))
            })
    })?;
    Some(Home {
        building: plan.building,
        level,
        number: room.number,
        rect,
        door: room.door?,
        rig,
        bed_side,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::validate_floor;
    use crate::tile::Terrain;
    use crate::{GenParams, Generator, SizePreset};

    fn medium(seed: u64) -> City {
        Generator::run(GenParams {
            seed,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn the_home_is_a_whole_flat_high_in_the_megablock() {
        for c in [medium(1), medium(2), medium(8), medium(11)] {
            let h = home(&c).expect("a home");
            let plan = FloorPlan::new(&c, h.building).unwrap();
            assert_eq!(plan.home_level, Some(h.level));
            assert!(h.level >= 2 && h.level < plan.top && Some(h.level) != plan.bridge);
            let f = generate_floor(&c, &plan, h.level);
            validate_floor(&plan, &f).unwrap();
            let room = f.rooms.iter().find(|r| r.kind == RoomKind::Home).unwrap();
            assert!(room.private && room.door == Some(h.door) && room.rect == h.rect);
            assert_eq!((h.rect.w, h.rect.h), (35, 7));
            assert_eq!(room.number as i32 / 100, h.level);
            let has = |x: Furniture| {
                h.rect
                    .iter()
                    .any(|p| f.tiles[p].feature == Some(Feature::Furniture(x)))
            };
            for x in [
                Furniture::Bed,
                Furniture::Desk,
                Furniture::Couch,
                Furniture::Table,
                Furniture::Shower,
                Furniture::Counter,
                Furniture::Shelf,
                Furniture::Locker,
            ] {
                assert!(has(x), "{x:?}");
            }
            assert_eq!(f.tiles[h.rig].feature, Some(Feature::Terminal));
            assert!(
                h.rig
                    .neighbors8()
                    .any(|n| h.rect.contains(n) && f.tiles[n].is_walkable())
            );
            // The living room has windows: the outer wall along the home is windowed.
            assert!(
                (h.rect.x..h.rect.right())
                    .any(|x| f.tiles[Pos::new(x, h.rect.bottom())].terrain == Terrain::Window)
            );
            assert!(f.tiles[h.bed_side].is_walkable() && h.rect.contains(h.bed_side));
            assert!(f.rooms.iter().filter(|r| r.kind == RoomKind::Home).count() == 1);
            assert!(
                !f.stations.iter().any(|s| h.rect.contains(s.pos)),
                "no stations at home"
            );
            assert_eq!(home(&c), Some(h)); // deterministic
        }
    }

    #[test]
    fn only_the_first_atrium_megablock_has_a_home_level() {
        for c in [medium(1), medium(8)] {
            let with: Vec<_> = c
                .buildings
                .iter()
                .filter_map(|b| FloorPlan::new(&c, b.id))
                .filter(|p| p.home_level.is_some())
                .collect();
            assert_eq!(with.len(), 1);
            assert!(with[0].atrium.is_some());
        }
    }

    #[test]
    fn a_narrow_megablock_walls_off_the_sides_no_door_could_reach() {
        // A hand-narrowed copy: 45 wide leaves three tiles either side of the home,
        // short of the walkway.
        let mut c = medium(8);
        let b = c
            .buildings
            .iter()
            .position(|b| FloorPlan::new(&c, b.id).is_some_and(|p| p.home_level.is_some()))
            .unwrap();
        c.buildings[b].footprint.w = 15;
        let h = home(&c).expect("a home");
        assert_eq!(h.building, c.buildings[b].id);
        let plan = FloorPlan::new(&c, h.building).unwrap();
        let f = generate_floor(&c, &plan, h.level);
        validate_floor(&plan, &f).unwrap();
        for x in (1..h.rect.x).chain(h.rect.right()..plan.size.w as i32 - 1) {
            for y in h.rect.y..h.rect.bottom() {
                assert_eq!(f.tiles[Pos::new(x, y)].terrain, Terrain::Wall, "{x},{y}");
            }
        }
    }
}
