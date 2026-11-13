//! The ward: a hand-made two-level building that gives the guards' rules exact
//! coordinates (the plan's Ruling 18).
//!
//! ```text
//!      0         1         2         3
//!      01234567890123456789012345678901
//! y0   ################################
//! y1   #.S.........LL.#........#......#
//! y2   #..............#........#......#
//! y3   X..............K........P......#
//! y4   #..............#........#......#
//! y5   #..............#........#......#
//! y6   ################################
//! ```
//!
//! `X` an exit onto the street, `K` a card lock (zone 1, free from its east side), `P`
//! the door of a private office (zone 2). The east hall is dark. Level 1 (`2F`) is one
//! lit hall with the same stair and lift.

use crate::app::AppState;
use crate::foes::{Foe, Post};
use citygen::venue::{LockKind, Ways};
use citygen::{
    BuildingId, BuildingKind, City, DoorLock, Exit, Floor, FloorPlan, Grid, LevelLink, LinkKind,
    LinkStop, Place, Pos, Rect, Room, RoomKind, Size, StratumKind, Terrain, Theme, Tile,
};

/// The ward's building id, on the lot (which has no buildings of its own).
pub(crate) const WARD: BuildingId = BuildingId(0);

const SIZE: Size = Size { w: 32, h: 7 };
/// The light of the lit tiles, and of the dark east hall.
const LIT: u8 = 150;
const DARK: u8 = 10;

/// A floor of the ward: walls round, floor inside, the stair and the lift bank, lit
/// by `light` at each tile.
fn frame(level: i32, light: impl Fn(Pos) -> u8) -> Floor {
    let mut tiles = Grid::filled(SIZE, Tile::new(Terrain::Floor));
    let (r, b) = (SIZE.w as i32 - 1, SIZE.h as i32 - 1);
    for y in 0..=b {
        for x in 0..=r {
            if x == 0 || y == 0 || x == r || y == b {
                tiles.set(Pos::new(x, y), Tile::new(Terrain::Wall));
            }
        }
    }
    tiles.set(Pos::new(2, 1), Tile::new(Terrain::Stair));
    tiles.set(Pos::new(12, 1), Tile::new(Terrain::Lift));
    tiles.set(Pos::new(13, 1), Tile::new(Terrain::Lift));
    let mut lights = Grid::filled(SIZE, 0);
    let mut glow = Grid::filled(SIZE, (0, 0, 0));
    for (p, _) in tiles.iter() {
        let v = light(p);
        lights.set(p, v);
        glow.set(p, (v, v, v));
    }
    Floor {
        level,
        tiles,
        light: lights,
        glow,
        rooms: Vec::new(),
        exits: Vec::new(),
        stations: Vec::new(),
        locks: Vec::new(),
    }
}

/// L: the west hall, the card lock into the dark east hall, the private office.
fn ground() -> Floor {
    let mut f = frame(0, |p| if (16..=23).contains(&p.x) { DARK } else { LIT });
    for y in 1..=5 {
        f.tiles.set(Pos::new(15, y), Tile::new(Terrain::Wall));
        f.tiles.set(Pos::new(24, y), Tile::new(Terrain::Wall));
    }
    for door in [Pos::new(0, 3), Pos::new(15, 3), Pos::new(24, 3)] {
        f.tiles.set(door, Tile::new(Terrain::Door));
    }
    f.exits.push(Exit {
        at: Pos::new(0, 3),
        stratum: StratumKind::Street,
        landing: Pos::new(6, 8),
        door: Pos::new(6, 9),
    });
    f.locks.push(DoorLock {
        at: Pos::new(15, 3),
        number: 1,
        zone: 1,
        kind: LockKind::Card,
        ways: Ways::CARD,
        free_from: Some(Pos::new(16, 3)),
    });
    let room = |rect, kind, zone| Room {
        zone,
        ..Room::new(rect, kind, None, false, 0)
    };
    f.rooms = vec![
        room(Rect::new(1, 1, 14, 5), RoomKind::Hall, 0),
        room(Rect::new(16, 1, 8, 5), RoomKind::Corridor, 1),
        Room {
            zone: 2,
            ..Room::new(
                Rect::new(25, 1, 6, 5),
                RoomKind::Office,
                Some(Pos::new(24, 3)),
                true,
                7,
            )
        },
    ];
    f
}

/// 2F: one lit hall.
fn upstairs() -> Floor {
    let mut f = frame(1, |_| LIT);
    f.rooms = vec![Room::new(
        Rect::new(1, 1, 30, 5),
        RoomKind::Hall,
        None,
        false,
        0,
    )];
    f
}

/// A stop of a link on `level` at `at`, on and off.
fn stop(level: i32, at: Pos, card: u8) -> LinkStop {
    LinkStop {
        level,
        at,
        card,
        on: true,
        off: true,
    }
}

/// The ward's plan: two levels, a stair and a lift whose 2F stop asks card 1.
fn plan() -> FloorPlan {
    let link = |kind, stops| LevelLink {
        kind,
        stops,
        name: None,
    };
    FloorPlan {
        building: WARD,
        kind: BuildingKind::Office,
        theme: Theme::CorpCore,
        footprint: Rect::new(0, 0, 32, 7),
        size: SIZE,
        levels: vec![0, 1],
        doors: Vec::new(),
        links: vec![
            link(
                LinkKind::Stair,
                vec![stop(0, Pos::new(2, 1), 0), stop(1, Pos::new(2, 1), 0)],
            ),
            link(
                LinkKind::Lift,
                vec![stop(0, Pos::new(12, 1), 0), stop(1, Pos::new(12, 1), 1)],
            ),
        ],
        atrium: None,
        home_level: None,
        clinic: None,
        sizes: Vec::new(),
        crawls: Vec::new(),
        venue: None,
    }
}

/// The lot with the ward's plan and both its floors, you on L at (6,3) with your
/// sight refreshed, and no foes.
pub(crate) fn ward() -> AppState {
    let mut st = AppState::new();
    st.enter_city(City::lot(1));
    st.interiors.put_plan(plan());
    st.interiors.put_floor(WARD, ground());
    st.interiors.put_floor(WARD, upstairs());
    st.foes.clear();
    st.player = Some(crate::walker::Player::at(on(0), Pos::new(6, 3)));
    stand(&mut st, 0, Pos::new(6, 3));
    st
}

/// The ward's floor `level`.
pub(crate) fn on(level: i32) -> Place {
    Place::Floor {
        building: WARD,
        level,
    }
}

/// You on foot on the ward's floor `level` at `at`, your sight refreshed.
pub(crate) fn stand(st: &mut AppState, level: i32, at: Pos) {
    crate::constructs::land(st, on(level), at);
}

/// A post of the ward's: zone `zone`'s card and `patrol`.
pub(crate) fn post(zone: u8, patrol: &[Pos]) -> Post {
    Post::new(WARD, zone, patrol.to_vec())
}

/// Building `b` with no guards: every foe posted there is gone, for a test about
/// something else that stands in a copy while guards act.
pub(crate) fn no_guards(st: &mut AppState, b: BuildingId) {
    st.foes
        .retain(|f| f.post.as_ref().is_none_or(|p| p.building != b));
}

/// A guard of zone `zone` on the ward's floor `level`, standing on the first tile of
/// `patrol`: its index.
pub(crate) fn guard(st: &mut AppState, level: i32, zone: u8, patrol: &[Pos]) -> usize {
    let at = *patrol.first().expect("a patrol of a tile at least");
    st.foes.push(Foe {
        name: "guard".to_string(),
        glyph: '@',
        post: Some(post(zone, patrol)),
        ..Foe::goon(on(level), at)
    });
    st.foes.len() - 1
}

/// A street goon on the ward's floor `level` at `at`: its index.
pub(crate) fn goon(st: &mut AppState, level: i32, at: Pos) -> usize {
    st.foes.push(Foe::goon(on(level), at));
    st.foes.len() - 1
}

/// The clock moves `n` ticks: the app's and the living city's, which the clock reads
/// while there is a city.
pub(crate) fn advance(st: &mut AppState, n: u64) {
    st.tick += n;
    if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
        world.advance(city, world.tick() + n);
    }
}

/// `n` ticks of the clock, the guards' step after each.
pub(crate) fn run(st: &mut AppState, n: u64) {
    for _ in 0..n {
        advance(st, 1);
        crate::guards::tick(st);
    }
}

/// Ticks of the clock as `run`'s until `done`; panics when it isn't in `max`.
pub(crate) fn run_until(st: &mut AppState, max: u64, done: impl Fn(&AppState) -> bool) {
    for _ in 0..max {
        if done(st) {
            return;
        }
        run(st, 1);
    }
    assert!(done(st), "not done in {max} ticks");
}

/// Whether a line of the log holds `needle`.
pub(crate) fn logged(st: &AppState, needle: &str) -> bool {
    st.log.iter().any(|e| e.text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ward_is_laid_as_ruled() {
        let st = ward();
        let (plan, floor) = st.interiors.floor(WARD, 0).expect("its L");
        assert_eq!(plan.levels, vec![0, 1]);
        assert_eq!(plan.links[1].kind, LinkKind::Lift);
        assert_eq!(
            plan.links[1].stops[1].card, 1,
            "the lift's 2F stop asks card 1"
        );
        let at = |x, y| floor.tiles.get(Pos::new(x, y)).unwrap().terrain;
        assert_eq!(at(0, 3), Terrain::Door);
        assert_eq!(at(15, 3), Terrain::Door);
        assert_eq!(at(24, 3), Terrain::Door);
        assert_eq!(at(15, 2), Terrain::Wall);
        assert_eq!(at(2, 1), Terrain::Stair);
        assert_eq!(floor.light.get(Pos::new(20, 3)), Some(&DARK));
        assert_eq!(floor.light.get(Pos::new(26, 3)), Some(&LIT));
        assert!(floor.exit_at(Pos::new(0, 3)).is_some());
        assert_eq!(floor.door_of(Pos::new(24, 3)).map(|r| r.number), Some(7));
        assert!(st.interiors.floor(WARD, 1).is_some());
        assert!(st.foes.is_empty());
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (on(0), Pos::new(6, 3)));
        assert_eq!(
            p.vis().get(Pos::new(10, 3)),
            Some(&crate::render::Visibility::Visible)
        );
    }

    #[test]
    fn no_guards_drops_only_that_buildings_posted_foes() {
        let mut st = ward();
        guard(&mut st, 0, 1, &[Pos::new(20, 2)]);
        let goon = goon(&mut st, 0, Pos::new(10, 3));
        let elsewhere = Foe {
            post: Some(Post::new(BuildingId(1), 0, Vec::new())),
            ..Foe::goon(on(1), Pos::new(5, 3))
        };
        st.foes.push(elsewhere.clone());
        let kept = vec![st.foes[goon].clone(), elsewhere];
        no_guards(&mut st, WARD);
        assert_eq!(st.foes, kept);
    }

    #[test]
    fn a_guard_stands_on_its_patrols_first_tile_and_you_where_told() {
        let mut st = ward();
        let patrol = [Pos::new(20, 2), Pos::new(20, 4)];
        let g = guard(&mut st, 0, 1, &patrol);
        let f = &st.foes[g];
        assert_eq!((f.name.as_str(), f.glyph, f.pos), ("guard", '@', patrol[0]));
        assert_eq!(f.post, Some(post(1, &patrol)));
        stand(&mut st, 1, Pos::new(5, 3));
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), (on(1), Pos::new(5, 3)));
    }
}
