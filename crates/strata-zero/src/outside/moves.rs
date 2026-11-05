//! Moving on the overworld, going down onto a tile and coming back up, on the city's
//! clock: a step costs the walk across a tile of that ground.

use super::{At, TILE};
use crate::app::AppState;
use citygen::overworld::{Ground, Overworld, Water, detail};
use citygen::{Dir, Place, Pos, StratumKind};

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    Stepped,
    /// Refused, and why (a line for the log).
    Blocked(&'static str),
    /// Onto the city's street at this cell of its frame: in the city, or past its edge
    /// on the open ground.
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
    let (Some(o), Some(_)) = (st.outside.as_ref(), st.city.as_ref()) else {
        return Moved::Blocked("there is no world out here");
    };
    let At::Overworld(from) = o.at else {
        return Moved::Blocked("not on the overworld");
    };
    let to = from.step(d);
    // onto a city's or a town's block from outside it: into it
    if let Some(s) = crate::outside::settlement_at(st, to)
        && crate::outside::settlement_at(st, from) != Some(s)
    {
        match crate::outside::enter(st, s) {
            Ok(()) => {
                if let (Some(o), Some(city)) = (st.outside.as_ref(), st.city.as_ref()) {
                    return Moved::City(o.street_of(city, to));
                }
            }
            Err(why) => st.warn(why),
        }
    }
    let Some(o) = st.outside.as_ref() else {
        return Moved::Blocked("there is no world out here");
    };
    let Some(cost) = step_cost(o.world(), to) else {
        return Moved::Blocked(closed(o.world(), to));
    };
    if let Some(o) = st.outside.as_mut() {
        o.at = At::Overworld(to);
    }
    crate::clock::run(st, Some(cost), crate::clock::PATHS_PER_FRAME);
    crate::outside::reached_mark(st);
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

/// ↵ on the overworld: into the city or town that owns the tile (on its own street
/// nearest the tile's middle), else onto the tile's ground (on its road if it has one,
/// else the dry ground nearest its middle), on the city's street past its edge; ground
/// another city or town owns is handed over to it as you stand on it (`into_city`).
pub fn down(st: &mut AppState) -> Result<Moved, &'static str> {
    let (Some(o), Some(_)) = (st.outside.as_ref(), st.city.as_ref()) else {
        return Err("there is no world out here");
    };
    let At::Overworld(t) = o.at else {
        return Err("not on the overworld");
    };
    // whose tile it is, not whose map covers it: a town's own tile in its city's block
    // is the town's
    if let Some(s) = o.owner(t).or_else(|| crate::outside::settlement_at(st, t)) {
        match crate::outside::enter(st, s) {
            Ok(()) => {
                if let (Some(o), Some(city)) = (st.outside.as_ref(), st.city.as_ref()) {
                    return Ok(Moved::City(o.street_of(city, t)));
                }
            }
            Err(why) => st.warn(why),
        }
    }
    let Some(o) = st.outside.as_ref() else {
        return Err("there is no world out here");
    };
    // the road nearest the middle, else dry ground joined to the open (not shut in a
    // pen or a crater), else the nearest shore when the tile is under water
    let cell = detail::landing(o.world(), t).ok_or("nothing to stand on there")?;
    let city = st.city.as_ref().ok_or("there is no world out here")?;
    let origin = o.origin(city);
    Ok(Moved::City(Pos::new(cell.x - origin.x, cell.y - origin.y)))
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

/// Onto the city's street at `cell` of its frame: in the city, or past its edge on the
/// open ground; on a cell another city or town owns, into it there, as walking onto it
/// goes in.
pub fn into_city(st: &mut AppState, cell: Pos) {
    let street = Place::Stratum(StratumKind::Street);
    if st.player.is_none() {
        st.player = Some(crate::walker::Player::at(street, cell));
    }
    if let Some(p) = st.player.as_mut() {
        p.place = street;
        p.pos = cell;
        p.body = None;
    }
    if let Some(o) = st.outside.as_mut() {
        o.at = At::City;
    }
    // you see where you've come down, not where you left (a handover looks round)
    if crate::outside::stream::hand_over(st).is_none() {
        crate::screens::explore::look_round(st);
    }
    // the city's MAP is the city's again
    if st.atlas.zoom == crate::screens::atlas::Zoom::World {
        st.atlas.zoom = crate::screens::atlas::Zoom::Fit;
    }
    crate::outside::reached_mark(st);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::outside::ensure;
    use citygen::Terrain;

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
        world(st)
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| !crate::outside::in_any_block(st, *p) && f(*p))
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

    /// The world cell of the live city's cell `p`.
    fn world_cell(st: &AppState, p: Pos) -> Pos {
        let o = st.outside.as_ref().unwrap();
        o.origin(st.city.as_ref().unwrap()).offset(p.x, p.y)
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
        let Ok(Moved::City(cell)) = down(&mut st) else {
            panic!("onto the street past the city's edge")
        };
        let c = world_cell(&st, cell);
        let g = detail::ground(&w, road.x, road.y);
        assert_eq!(
            g[Pos::new(c.x - TILE * road.x, c.y - TILE * road.y)].terrain,
            Terrain::Road
        );
        let mut landed = 0;
        for (i, (p, t)) in w.tiles.iter().enumerate() {
            if i % 7 != 0
                || b.contains(p)
                || crate::outside::in_any_block(&st, p)
                || matches!(t.water, Water::Sea | Water::Lake)
            {
                continue;
            }
            put(&mut st, At::Overworld(p));
            match down(&mut st) {
                Ok(Moved::City(cell)) => {
                    let c = world_cell(&st, cell);
                    let r = detail::region(&w, c.x, c.y, 1, 1);
                    let cell = r[Pos::new(0, 0)];
                    assert!(cell.is_walkable(), "{p:?}: {:?}", cell.terrain);
                    assert!(!matches!(cell.terrain, Terrain::Water | Terrain::Shallows));
                    let landed_on = Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE));
                    assert!(
                        (landed_on.x - p.x).abs() <= 1 && (landed_on.y - p.y).abs() <= 1,
                        "{p:?} → {landed_on:?}"
                    );
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
    fn down_onto_the_ground_is_the_citys_street_past_its_edge() {
        let mut st = on_the_world();
        let w = world(&st).clone();
        let road = find(&st, |p| {
            w.tile(p)
                .is_some_and(|t| t.road != 0 && t.water == Water::None)
        });
        put(&mut st, At::Overworld(road));
        let Ok(Moved::City(cell)) = down(&mut st) else {
            panic!("onto the street")
        };
        // still on the overworld until it's stood on
        assert_eq!(st.outside.as_ref().unwrap().at, At::Overworld(road));
        into_city(&mut st, cell);
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos),
            (Place::Stratum(StratumKind::Street), cell)
        );
        assert!(!st.city.as_ref().unwrap().size.contains(cell));
        assert!(p.beyond_sight().len() > 50, "you see the ground round you");
    }

    #[test]
    fn up_from_the_ground_is_the_tile_you_are_on() {
        let mut st = on_the_world();
        // on the street past the city's edge: the tile its world cell lies in
        let city = st.city.clone().unwrap();
        let origin = st.outside.as_ref().unwrap().origin(&city);
        let cell = Pos::new(8 * 12 + 5 - origin.x, 8 * 7 + 2 - origin.y);
        st.player.as_mut().unwrap().pos = cell;
        up(&mut st);
        assert_eq!(
            st.outside.as_ref().unwrap().at,
            At::Overworld(Pos::new(12, 7))
        );
        // from the city: the block tile your street cell lies in
        put(&mut st, At::City);
        let cell = st.city.as_ref().unwrap().size.center();
        st.player.as_mut().unwrap().pos = cell;
        up(&mut st);
        let o = st.outside.as_ref().unwrap();
        assert_eq!(o.at, At::Overworld(o.tile_of(&city, cell)));
    }

    /// The overworld tile `t`'s owner and you landing there by ↵: where you stand.
    fn land(st: &mut AppState, t: Pos) -> Option<crate::outside::Settlement> {
        put(st, At::Overworld(t));
        let Ok(Moved::City(cell)) = down(st) else {
            return None;
        };
        into_city(st, cell);
        Some(st.outside.as_ref().unwrap().here())
    }

    /// Whether you stand on the live city's own cells, on its street.
    fn on_its_own_street(st: &AppState) -> bool {
        let p = st.player.as_ref().unwrap();
        crate::outside::stream::live_owns(st, p.pos)
            && st
                .city
                .as_ref()
                .unwrap()
                .walkable(StratumKind::Street, p.pos)
    }

    #[test]
    fn down_on_a_towns_tile_is_into_the_town() {
        // a town's own tile inside its city's block: ↵ there goes into the town
        use crate::outside::stream::tests::beyond::city_with_a_town;
        let mut st = crate::outside::tests::home_run();
        let (c, town) = city_with_a_town(&st);
        let t = {
            let o = st.outside.as_ref().unwrap();
            let block = o.others().into_iter().find(|(s, _)| *s == c).unwrap().1;
            block
                .iter()
                .find(|t| o.owner(*t) == Some(town) && step_cost(o.world(), *t).is_some())
                .unwrap()
        };
        assert_eq!(land(&mut st, t), Some(town));
        assert!(on_its_own_street(&st));
        let p = st.player.as_ref().unwrap();
        let at = world_cell(&st, p.pos);
        assert_eq!(
            st.outside
                .as_ref()
                .unwrap()
                .owner(Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE))),
            Some(town)
        );
    }

    #[test]
    fn coming_down_on_a_cell_another_city_owns_is_into_it() {
        // put on the street on a cell another city owns (as ↵ onto the ground beside
        // its map can land you): handed over to it, on the same cell
        use crate::outside::stream::tests::handover::{beside_a_city, world_of};
        let mut st = crate::outside::tests::home_run();
        let (s, x, d) = beside_a_city(&mut st);
        let at = world_of(&st, x.step(d));
        into_city(&mut st, x.step(d));
        assert_eq!(st.outside.as_ref().unwrap().here(), s);
        assert_eq!(world_of(&st, st.player.as_ref().unwrap().pos), at);
        assert!(on_its_own_street(&st));
    }

    #[test]
    fn down_on_a_tile_of_the_live_map_another_owns_is_into_it() {
        // a town's own tile inside the live city's block: ↵ there is into the town, not
        // onto the live city's map there (the ground under the town's buildings), drawn
        // from the town's
        use crate::outside::stream::tests::beyond::city_with_a_town;
        let mut st = crate::outside::tests::home_run();
        let (c, town) = city_with_a_town(&st);
        crate::outside::enter(&mut st, c).unwrap();
        let t = {
            let o = st.outside.as_ref().unwrap();
            let b = o.block(st.city.as_ref().unwrap());
            b.iter()
                .find(|t| o.owner(*t) == Some(town) && step_cost(o.world(), *t).is_some())
                .unwrap()
        };
        assert_eq!(
            crate::outside::settlement_at(&st, t),
            Some(c),
            "the live map's"
        );
        assert_eq!(land(&mut st, t), Some(town));
        assert!(on_its_own_street(&st));
    }
}
