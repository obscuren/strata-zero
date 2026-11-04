//! Moving on the overworld, going down onto a tile and coming back up, on the city's
//! clock: a step costs the walk across a tile of that ground.

use super::{At, TILE, nearest};
use crate::app::AppState;
use citygen::overworld::{Ground, Overworld, Water, detail};
use citygen::{Dir, Place, Pos, StratumKind, Terrain};

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    Stepped,
    /// Refused, and why (a line for the log).
    Blocked(&'static str),
    /// Into the city, on this street cell.
    City(Pos),
}

/// What walking onto overworld tile `to` costs, in world ticks; `None` where you can't:
/// the walk across a tile on foot, twice that in the hills, four times in the mountains,
/// twice more wading a river with no road over it. Peaks, the sea and lakes are closed.
pub fn step_cost(world: &Overworld, to: Pos) -> Option<u64> {
    let t = world.tile(to)?;
    let ground = match t.ground {
        Ground::Peak => return None,
        Ground::Mountains => 4,
        Ground::Hills => 2,
        _ => 1,
    };
    let water = match t.water {
        Water::Sea | Water::Lake => return None,
        Water::River if t.road == 0 => 2,
        _ => 1,
    };
    Some(TILE as u64 * sim::WALK_TICKS * ground * water)
}

/// One step across the overworld: onto the next tile on the city's clock, or into the
/// city when the step crosses into its block from outside.
pub fn world_step(st: &mut AppState, d: Dir) -> Moved {
    let (Some(o), Some(city)) = (st.outside.as_ref(), st.city.as_ref()) else {
        return Moved::Blocked("there is no world out here");
    };
    let At::Overworld(from) = o.at else {
        return Moved::Blocked("not on the overworld");
    };
    let to = from.step(d);
    let block = o.block(city);
    if block.contains(to) && !block.contains(from) {
        return Moved::City(o.street_of(city, to));
    }
    let Some(cost) = step_cost(o.world(), to) else {
        return Moved::Blocked(closed(o.world(), to));
    };
    if let Some(o) = st.outside.as_mut() {
        o.at = At::Overworld(to);
    }
    crate::clock::run(st, Some(cost), crate::clock::PATHS_PER_FRAME);
    Moved::Stepped
}

/// Why a tile can't be walked onto.
fn closed(world: &Overworld, to: Pos) -> &'static str {
    match world.tile(to) {
        None => "the world ends here",
        Some(t) if t.water == Water::Sea => "open sea",
        Some(t) if t.water == Water::Lake => "a lake",
        Some(_) => "too steep to climb",
    }
}

/// ↵ on the overworld: onto the tile's ground (on its road if it has one, else the dry
/// ground nearest its middle), or into the city on a tile of its block.
pub fn down(st: &mut AppState) -> Result<Moved, &'static str> {
    let (Some(o), Some(city)) = (st.outside.as_ref(), st.city.as_ref()) else {
        return Err("there is no world out here");
    };
    let At::Overworld(t) = o.at else {
        return Err("not on the overworld");
    };
    if o.block(city).contains(t) {
        return Ok(Moved::City(o.street_of(city, t)));
    }
    let g = detail::ground(o.world(), t.x, t.y);
    let mid = Pos::new(TILE / 2, TILE / 2);
    let on = |want: fn(&citygen::Tile) -> bool| nearest(mid, TILE, |p| g.get(p).is_some_and(want));
    let spot = on(|c| c.terrain == Terrain::Road)
        .or_else(|| {
            on(|c| c.is_walkable() && !matches!(c.terrain, Terrain::Water | Terrain::Shallows))
        })
        .ok_or("nothing to stand on there")?;
    let cell = Pos::new(TILE * t.x + spot.x, TILE * t.y + spot.y);
    if let Some(o) = st.outside.as_mut() {
        o.at = At::Ground(cell);
    }
    Ok(Moved::Stepped)
}

/// Up onto the overworld: from the open ground onto the tile you're on, from the city's
/// street onto the block tile you're in.
pub fn up(st: &mut AppState) {
    let Some(o) = st.outside.as_ref() else {
        return;
    };
    let tile = match (o.at, st.city.as_ref(), st.player.as_ref()) {
        (At::Ground(c), _, _) => Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
        (At::City, Some(city), Some(p)) => o.tile_of(city, p.pos),
        _ => return,
    };
    if let Some(o) = st.outside.as_mut() {
        o.at = At::Overworld(tile);
    }
}

/// Into the city on its street at `cell`.
pub fn into_city(st: &mut AppState, cell: Pos) {
    let street = Place::Stratum(StratumKind::Street);
    match st.player.as_mut() {
        Some(p) => {
            p.place = street;
            p.pos = cell;
        }
        None => st.player = Some(crate::walker::Player::at(street, cell)),
    }
    if let Some(o) = st.outside.as_mut() {
        o.at = At::City;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::outside::ensure;

    /// The seed-8 medium city with its world, you on the overworld at `tile`.
    pub(crate) fn on_the_world() -> AppState {
        let mut st = AppState::new();
        st.enter_city(crate::runs::testutil::story_city());
        let city = st.city.clone().unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        ensure(&mut st);
        st
    }

    fn block(st: &AppState) -> citygen::Rect {
        st.outside
            .as_ref()
            .unwrap()
            .block(st.city.as_ref().unwrap())
    }

    fn world(st: &AppState) -> &Overworld {
        st.outside.as_ref().unwrap().world()
    }

    fn put(st: &mut AppState, at: At) {
        st.outside.as_mut().unwrap().at = at;
    }

    fn ticks(st: &AppState) -> u64 {
        st.world.as_ref().unwrap().tick()
    }

    /// A tile outside the block where `f` holds, with a neighbour on `d` that also does.
    fn find(st: &AppState, f: impl Fn(Pos) -> bool) -> Pos {
        let b = block(st);
        world(st)
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| !b.contains(*p) && f(*p))
            .expect("such a tile")
    }

    #[test]
    fn a_step_costs_the_walk_across_a_tile() {
        let mut st = on_the_world();
        for (ground, times) in [(Ground::Flats, 1), (Ground::Hills, 2)] {
            let w = world(&st).clone();
            let plain = |p: Pos| {
                w.tile(p)
                    .is_some_and(|t| t.ground == ground && t.water == Water::None && t.road == 0)
            };
            let b = block(&st);
            let from = find(&st, |p| {
                plain(p) && plain(p.offset(1, 0)) && !b.contains(p.offset(1, 0))
            });
            put(&mut st, At::Overworld(from));
            let before = ticks(&st);
            assert_eq!(world_step(&mut st, Dir::E), Moved::Stepped);
            assert_eq!(
                ticks(&st) - before,
                times * 8 * sim::WALK_TICKS,
                "{ground:?}"
            );
            assert_eq!(
                st.outside.as_ref().unwrap().at,
                At::Overworld(from.offset(1, 0))
            );
        }
    }

    #[test]
    fn the_sea_lakes_and_peaks_are_closed() {
        let mut st = on_the_world();
        let w = world(&st).clone();
        // seed 8's world is landlocked: its closed tiles are lakes and peaks
        let shut = |p: Pos| w.tile(p).is_some() && step_cost(&w, p).is_none();
        let from = find(&st, |p| {
            step_cost(&w, p).is_some()
                && [Dir::N, Dir::E, Dir::S, Dir::W]
                    .iter()
                    .any(|d| shut(p.step(*d)))
        });
        let d = [Dir::N, Dir::E, Dir::S, Dir::W]
            .into_iter()
            .find(|d| shut(from.step(*d)))
            .unwrap();
        put(&mut st, At::Overworld(from));
        let before = ticks(&st);
        assert!(matches!(world_step(&mut st, d), Moved::Blocked(_)));
        assert_eq!(st.outside.as_ref().unwrap().at, At::Overworld(from));
        assert_eq!(ticks(&st), before, "a refused step costs nothing");
        assert_eq!(step_cost(&w, Pos::new(-1, 0)), None, "off the world");
        for (p, t) in w.tiles.iter() {
            let open = step_cost(&w, p).is_some();
            let should = !matches!(t.water, Water::Sea | Water::Lake) && t.ground != Ground::Peak;
            assert_eq!(open, should, "{p:?}");
        }
    }

    #[test]
    fn stepping_onto_the_block_goes_into_the_city() {
        let mut st = on_the_world();
        let b = block(&st);
        let w = world(&st).clone();
        // a tile beside the block you can stand on, and the step into it
        let (from, d) = (b.y..b.y + b.h)
            .flat_map(|y| {
                [
                    (Pos::new(b.x - 1, y), Dir::E),
                    (Pos::new(b.x + b.w, y), Dir::W),
                ]
            })
            .chain((b.x..b.x + b.w).flat_map(|x| {
                [
                    (Pos::new(x, b.y - 1), Dir::S),
                    (Pos::new(x, b.y + b.h), Dir::N),
                ]
            }))
            .find(|(p, d)| step_cost(&w, *p).is_some() && step_cost(&w, p.step(*d)).is_some())
            .expect("a way into the block");
        put(&mut st, At::Overworld(from));
        let Moved::City(cell) = world_step(&mut st, d) else {
            panic!("into the city")
        };
        into_city(&mut st, cell);
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos),
            (Place::Stratum(StratumKind::Street), cell)
        );
        let t = st
            .city
            .as_ref()
            .unwrap()
            .tile(StratumKind::Street, cell)
            .unwrap();
        assert!(t.is_walkable() && t.building.is_none());
    }

    #[test]
    fn inside_the_block_the_overworld_moves_you() {
        let mut st = on_the_world();
        let b = block(&st);
        let w = world(&st).clone();
        let from = (b.y..b.y + b.h)
            .flat_map(|y| (b.x..b.x + b.w - 1).map(move |x| Pos::new(x, y)))
            .find(|p| step_cost(&w, *p).is_some() && step_cost(&w, p.offset(1, 0)).is_some())
            .unwrap();
        put(&mut st, At::Overworld(from));
        assert_eq!(world_step(&mut st, Dir::E), Moved::Stepped);
        // and ↵ there goes down into the city
        assert!(matches!(down(&mut st), Ok(Moved::City(_))));
    }

    #[test]
    fn down_lands_on_the_road_or_dry_ground() {
        let mut st = on_the_world();
        let w = world(&st).clone();
        let b = block(&st);
        let road = find(&st, |p| {
            w.tile(p)
                .is_some_and(|t| t.road != 0 && t.water == Water::None)
        });
        put(&mut st, At::Overworld(road));
        assert_eq!(down(&mut st), Ok(Moved::Stepped));
        let At::Ground(c) = st.outside.as_ref().unwrap().at else {
            panic!("on the ground")
        };
        let g = detail::ground(&w, road.x, road.y);
        assert_eq!(
            g[Pos::new(c.x - TILE * road.x, c.y - TILE * road.y)].terrain,
            Terrain::Road
        );
        let mut landed = 0;
        for (i, (p, t)) in w.tiles.iter().enumerate() {
            if i % 7 != 0 || b.contains(p) || matches!(t.water, Water::Sea | Water::Lake) {
                continue;
            }
            put(&mut st, At::Overworld(p));
            match down(&mut st) {
                Ok(Moved::Stepped) => {
                    let At::Ground(c) = st.outside.as_ref().unwrap().at else {
                        panic!()
                    };
                    let r = detail::region(&w, c.x, c.y, 1, 1);
                    let cell = r[Pos::new(0, 0)];
                    assert!(cell.is_walkable(), "{p:?}: {:?}", cell.terrain);
                    assert!(!matches!(cell.terrain, Terrain::Water | Terrain::Shallows));
                    assert_eq!((c.x.div_euclid(TILE), c.y.div_euclid(TILE)), (p.x, p.y));
                    landed += 1;
                }
                other => assert!(
                    matches!(t.ground, Ground::Peak | Ground::Mountains),
                    "{p:?} {:?}: {other:?}",
                    t.ground
                ),
            }
        }
        assert!(landed > 500, "{landed}");
    }

    #[test]
    fn up_from_the_ground_is_the_tile_you_are_on() {
        let mut st = on_the_world();
        put(&mut st, At::Ground(Pos::new(8 * 12 + 5, 8 * 7 + 2)));
        up(&mut st);
        assert_eq!(
            st.outside.as_ref().unwrap().at,
            At::Overworld(Pos::new(12, 7))
        );
        // from the city: the block tile your street cell lies in
        put(&mut st, At::City);
        let cell = st.player.as_ref().unwrap().pos;
        up(&mut st);
        let o = st.outside.as_ref().unwrap();
        assert_eq!(
            o.at,
            At::Overworld(o.tile_of(st.city.as_ref().unwrap(), cell))
        );
    }
}
