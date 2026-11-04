//! Bots out with you: what a bay in your chrome holds, on the map beside you. A bot
//! drifts near you, follows you about, comes with you from place to place and takes a
//! seat on turns. What a bot is (its body, its look, its turn) is content; this is the
//! entity.

use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use citygen::pathing::{astar, bresenham};
use citygen::rng::hash64;
use citygen::{Place, Pos, Terrain};
use game::{BotBody, ItemId, Site};

/// How far a bot strays from you, in tiles.
pub const LEASH: i32 = 2;
/// Ticks a bot waits between drifts: at least this, and up to `DRIFT_SPAN` more.
pub const DRIFT_MIN: u64 = 15;
pub const DRIFT_SPAN: u64 = 26;
/// Ticks between a bot's steps while it catches up.
pub const CATCH_UP_TICKS: u64 = 3;
/// Ticks a wake's dot lasts: three beats.
pub const WAKE_TICKS: u64 = 9;
/// Past this far it doesn't fly the way back: it is beside you again.
const LOST: i32 = 24;

/// A bot out with you.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BotOut {
    /// What it is.
    pub item: ItemId,
    /// The site and socket it came out of.
    pub dock: (Site, usize),
    pub place: Place,
    /// Where it is; `None` while there is nowhere free beside you.
    pub pos: Option<Pos>,
    /// The tick it last moved.
    pub moved_at: u64,
    /// The tick it drifts next.
    pub next_drift: u64,
    /// On its way back to you: it is far, out of sight of you or where it can't stay.
    pub catching_up: bool,
    /// The tiles it left lately, with the tick it left each, newest last.
    pub wake: Vec<(Pos, u64)>,
}

/// Whoever bots stay with: your body while you are jacked in, else you.
pub fn anchor(st: &AppState) -> Option<(Place, Pos)> {
    let p = st.player.as_ref()?;
    Some(p.body.unwrap_or((p.place, p.pos)))
}

/// The bot's own content.
pub fn spec(st: &AppState, i: usize) -> Option<&game::Bot> {
    st.content.item(&st.bots.get(i)?.item)?.bot.as_ref()
}

fn map<'a>(st: &'a AppState, place: Place) -> Option<MapRef<'a>> {
    MapRef::for_place(
        st.city.as_ref()?,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    )
}

/// A tile a body may enter. Air and ground both keep to what you could walk on for
/// now; a rover's rule is its own when it differs.
fn enters(body: BotBody, map: &MapRef, p: Pos) -> bool {
    match body {
        BotBody::Air | BotBody::Ground => map.walkable(p),
    }
}

fn body(st: &AppState, i: usize) -> BotBody {
    spec(st, i).map_or(BotBody::Air, |b| b.body)
}

/// A tile bot `me` may be on: one its body enters, not the anchor's, not a standing
/// foe's, not another bot's.
pub fn free(st: &AppState, place: Place, pos: Pos, me: usize) -> bool {
    let Some(m) = map(st, place) else {
        return false;
    };
    enters(body(st, me), &m, pos)
        && anchor(st).is_none_or(|a| a != (place, pos))
        && crate::foes::foe_at(st, place, pos).is_none()
        && !st
            .bots
            .iter()
            .enumerate()
            .any(|(j, b)| j != me && b.place == place && b.pos == Some(pos))
}

/// A tile is with the anchor at `to`: within the leash, and nothing that blocks sight
/// between them.
pub fn with_you(st: &AppState, place: Place, from: Pos, to: Pos) -> bool {
    if from.chebyshev(to) > LEASH {
        return false;
    }
    let Some(m) = map(st, place) else {
        return false;
    };
    bresenham(from, to)
        .into_iter()
        .filter(|p| *p != from && *p != to)
        .all(|p| m.tile(p).is_none_or(|t| !crate::foes::blocks_sight(t)))
}

fn door(st: &AppState, place: Place, p: Pos) -> bool {
    map(st, place)
        .and_then(|m| m.tile(p).map(|t| t.terrain == Terrain::Door))
        .unwrap_or(false)
}

/// Somewhere a bot can stay: free, with you, and not a doorway.
fn stays(st: &AppState, place: Place, p: Pos, at: Pos, me: usize) -> bool {
    free(st, place, p, me) && with_you(st, place, p, at) && !door(st, place, p)
}

/// The nearest tile beside the anchor where bot `me` can stay, ring by ring.
fn beside(st: &AppState, place: Place, at: Pos, me: usize) -> Option<Pos> {
    (1..=LEASH).find_map(|r| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| Pos::new(at.x + dx, at.y + dy)))
            .filter(|p| p.chebyshev(at) == r)
            .find(|p| stays(st, place, *p, at, me))
    })
}

fn drift_wait(now: u64, me: usize) -> u64 {
    DRIFT_MIN + hash64(now, me as u64 + 1) % DRIFT_SPAN
}

/// Makes `bots` what is docked: a bot docked and not out comes out beside you, one no
/// longer docked is gone, and one on another place than yours comes over to you.
pub fn sync(st: &mut AppState) {
    let docked = st.character.docked(&st.content);
    st.bots.retain(|b| {
        docked
            .iter()
            .any(|(s, n, id)| (*s, *n) == b.dock && *id == b.item)
    });
    let Some((place, at)) = anchor(st) else {
        st.bots.clear();
        return;
    };
    let now = st.clock_tick();
    for (site, n, item) in docked {
        if !st.bots.iter().any(|b| b.dock == (site, n)) {
            st.bots.push(BotOut {
                item,
                dock: (site, n),
                place,
                pos: None,
                moved_at: now,
                next_drift: now,
                catching_up: false,
                wake: Vec::new(),
            });
        }
    }
    for i in 0..st.bots.len() {
        let b = &st.bots[i];
        let lost = b.pos.is_none_or(|p| p.chebyshev(at) > LOST);
        if b.place == place && !lost {
            continue;
        }
        let spot = beside(st, place, at, i);
        let b = &mut st.bots[i];
        b.place = place;
        b.pos = spot;
        b.wake.clear();
        b.catching_up = false;
        b.moved_at = now;
        b.next_drift = now + drift_wait(now, i);
    }
}

/// From the frame loop: `sync`, then each bot ages its wake and, off turns, drifts or
/// catches up on the world's clock.
pub fn tick(st: &mut AppState) {
    sync(st);
    let now = st.clock_tick();
    for i in 0..st.bots.len() {
        st.bots[i].wake.retain(|&(_, t)| now < t + WAKE_TICKS);
        let place = st.bots[i].place;
        if st.net.engaged.as_ref().is_some_and(|e| e.place == place) {
            continue;
        }
        wander(st, i, now);
    }
}

/// One step toward somewhere bot `me` can stay: off a tile it can't stay on to a
/// neighbour it can, else along the shortest path its body flies toward the anchor.
/// `None` when there is no way.
pub fn toward(st: &AppState, me: usize) -> Option<Pos> {
    let b = st.bots.get(me)?;
    let (place, at) = anchor(st)?;
    let from = b.pos?;
    let near = (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| Pos::new(from.x + dx, from.y + dy)))
        .filter(|p| *p != from)
        .filter(|p| stays(st, place, *p, at, me))
        .min_by_key(|p| p.chebyshev(at));
    if near.is_some() {
        return near;
    }
    let m = map(st, place)?;
    let kind = body(st, me);
    let path = astar(m.size(), from, at, |_, to| {
        (to == at || (enters(kind, &m, to) && crate::foes::foe_at(st, place, to).is_none()))
            .then_some(1)
    })?;
    // The path runs four ways; a bot flies eight. It aims at the furthest point of the
    // path it can see along an open line, and steps that way, diagonals and all.
    let open = |q: Pos| {
        bresenham(from, q)
            .into_iter()
            .all(|t| t == from || t == at || enters(kind, &m, t))
    };
    let aim = path
        .iter()
        .copied()
        .take_while(|q| open(*q))
        .last()
        .unwrap_or(at);
    let toward_aim = Pos::new(
        from.x + (aim.x - from.x).signum(),
        from.y + (aim.y - from.y).signum(),
    );
    let next = path.into_iter().find(|p| *p != from && *p != at);
    [Some(toward_aim), next]
        .into_iter()
        .flatten()
        .find(|p| *p != from && *p != at && free(st, place, *p, me))
}

/// Moves bot `me` to `to`, leaving a wake where it was.
pub fn step_to(st: &mut AppState, me: usize, to: Pos, now: u64) {
    let wake = spec(st, me).is_some_and(|b| b.wake);
    let b = &mut st.bots[me];
    if let Some(from) = b.pos
        && wake
    {
        b.wake.push((from, now));
        let n = b.wake.len();
        if n > 3 {
            b.wake.drain(..n - 3);
        }
    }
    b.pos = Some(to);
    b.moved_at = now;
}

/// The bot out of the site and socket `dock`, by its place in `bots`.
pub fn index(st: &AppState, dock: (Site, usize)) -> Option<usize> {
    st.bots.iter().position(|b| b.dock == dock)
}

/// What the turn column calls bot `dock`: its item's name.
pub fn name(st: &AppState, dock: (Site, usize)) -> Option<String> {
    let b = &st.bots[index(st, dock)?];
    b.pos?;
    st.content.item(&b.item).map(|i| i.name.clone())
}

/// The bots placed on `place` with their reflex, in `bots` order: their seats on turns.
pub fn seats(st: &AppState, place: Place) -> Vec<(u8, (Site, usize))> {
    (0..st.bots.len())
        .filter(|&i| st.bots[i].place == place && st.bots[i].pos.is_some())
        .filter_map(|i| Some((spec(st, i)?.reflex, st.bots[i].dock)))
        .collect()
}

/// How far bot `dock` moves in a turn.
pub fn moves(st: &AppState, dock: (Site, usize)) -> u8 {
    index(st, dock)
        .and_then(|i| spec(st, i))
        .map_or(0, |b| b.moves)
}

/// A bot's turn begins: it acts when it is out, placed on the engaged map and away
/// from where it can stay; near you it passes. A bot that has gone passes too.
pub fn turn_start(st: &mut AppState, dock: (Site, usize)) -> bool {
    let Some(i) = index(st, dock) else {
        return false;
    };
    let Some((place, at)) = anchor(st) else {
        return false;
    };
    let b = &st.bots[i];
    let Some(here) = b.pos else {
        return false;
    };
    let engaged = st.net.engaged.as_ref().map(|e| e.place);
    if b.place != place || engaged != Some(place) || stays(st, place, here, at, i) {
        return false;
    }
    st.bots[i].catching_up = true;
    true
}

/// One action of a bot's turn: a step toward you while it is away from where it can
/// stay, else the turn is done. Combat fills the act half later.
pub fn step(st: &mut AppState, dock: (Site, usize), can_move: bool) -> crate::net::Step {
    use crate::net::Step;
    let Some(i) = index(st, dock) else {
        return Step::Nothing;
    };
    let done = |st: &mut AppState| {
        st.bots[i].catching_up = false;
        Step::Nothing
    };
    let Some((place, at)) = anchor(st) else {
        return done(st);
    };
    let Some(here) = st.bots[i].pos else {
        return done(st);
    };
    if !can_move || stays(st, place, here, at, i) {
        return done(st);
    }
    match toward(st, i) {
        Some(next) => {
            let now = st.clock_tick();
            step_to(st, i, next, now);
            Step::Moved(next)
        }
        None => done(st),
    }
}

/// Off turns: a bot that can stay where it is drifts now and then; one that can't
/// catches up, a step every `CATCH_UP_TICKS`.
fn wander(st: &mut AppState, me: usize, now: u64) {
    let Some((place, at)) = anchor(st) else {
        return;
    };
    let b = &st.bots[me];
    let Some(here) = b.pos else {
        return;
    };
    if b.place != place {
        return;
    }
    if !stays(st, place, here, at, me) {
        if now < b.moved_at + CATCH_UP_TICKS {
            return;
        }
        st.bots[me].catching_up = true;
        match toward(st, me) {
            Some(next) => step_to(st, me, next, now),
            // No way back: it is beside you again.
            None => st.bots[me].pos = beside(st, place, at, me),
        }
        return;
    }
    if b.catching_up {
        let b = &mut st.bots[me];
        b.catching_up = false;
        b.next_drift = now + drift_wait(now, me);
        return;
    }
    if now < b.next_drift {
        return;
    }
    let opts: Vec<Pos> = (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| Pos::new(here.x + dx, here.y + dy)))
        .filter(|p| *p != here && stays(st, place, *p, at, me))
        .collect();
    if !opts.is_empty() {
        let pick = opts[(hash64(now, me as u64 + 7) % opts.len() as u64) as usize];
        step_to(st, me, pick, now);
    }
    st.bots[me].next_drift = now + drift_wait(now, me);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use citygen::{City, Place, Pos, StratumKind, Terrain, Tile};

    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// The lot, you at (10, 8) with the debug runner's chrome: the cradle in, the
    /// Scout drone docked.
    fn lot() -> AppState {
        let mut st = AppState::new();
        st.enter_city(City::lot(1));
        st.character = game::Character::debug_runner(&st.content);
        st.player = Some(crate::walker::Player::at(STREET, Pos::new(10, 8)));
        st
    }

    fn you(st: &AppState) -> Pos {
        st.player.as_ref().unwrap().pos
    }

    fn pos(st: &AppState) -> Pos {
        st.bots[0].pos.expect("placed")
    }

    /// The world's clock on by `n` ticks, a bot tick each.
    fn run(st: &mut AppState, n: u64) {
        for _ in 0..n {
            let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
            world.advance(city, world.tick() + 1);
            tick(st);
        }
    }

    fn set(st: &mut AppState, p: Pos, t: Terrain) {
        *st.city
            .as_mut()
            .unwrap()
            .tile_mut(StratumKind::Street, p)
            .unwrap() = Tile::new(t);
    }

    #[test]
    fn a_docked_drone_comes_out_beside_you() {
        let mut st = lot();
        sync(&mut st);
        assert_eq!(st.bots.len(), 1);
        assert_eq!(st.bots[0].item, game::ItemId::new("scout_drone"));
        assert_eq!(st.bots[0].dock, (game::Site::Back, 0));
        assert_eq!(st.bots[0].place, STREET);
        assert_eq!(pos(&st).chebyshev(you(&st)), 1);
        assert!(free(&st, STREET, pos(&st), 0));
    }

    #[test]
    fn a_stowed_drone_goes() {
        let mut st = lot();
        sync(&mut st);
        st.character
            .chrome_copies
            .get_mut(&game::Site::Back)
            .unwrap()
            .fitted[0] = None;
        sync(&mut st);
        assert!(st.bots.is_empty());
    }

    #[test]
    fn a_swapped_drone_is_the_new_one() {
        let mut st = lot();
        sync(&mut st);
        let mule = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == game::ItemId::new("mule_drone"))
            .and_then(game::Stack::copy_id)
            .unwrap();
        let dock = game::Action::Fit {
            host: game::Host::Site(game::Site::Back),
            socket: 0,
            piece: mule,
        };
        crate::actions::perform(&mut st, dock).unwrap();
        sync(&mut st);
        assert_eq!(st.bots.len(), 1);
        assert_eq!(st.bots[0].item, game::ItemId::new("mule_drone"));
    }

    #[test]
    fn it_drifts_within_the_leash_and_off_your_tile() {
        let mut st = lot();
        sync(&mut st);
        let start = pos(&st);
        let mut moved = false;
        for _ in 0..2000 {
            run(&mut st, 1);
            let p = pos(&st);
            assert!(p.chebyshev(you(&st)) <= LEASH, "{p:?}");
            assert_ne!(p, you(&st));
            moved |= p != start;
        }
        assert!(moved, "it drifts");
    }

    #[test]
    fn it_never_drifts_through_a_door() {
        let mut st = lot();
        // A wall down x = 11 with a door beside you.
        for y in 1..18 {
            set(&mut st, Pos::new(11, y), Terrain::Wall);
        }
        set(&mut st, Pos::new(11, 8), Terrain::Door);
        sync(&mut st);
        for _ in 0..2000 {
            run(&mut st, 1);
            let p = pos(&st);
            assert!(p.x < 11, "through the door to {p:?}");
        }
    }

    #[test]
    fn it_catches_up() {
        let mut st = lot();
        sync(&mut st);
        st.player.as_mut().unwrap().pos = Pos::new(16, 8);
        run(&mut st, 3 * 8);
        assert!(with_you(&st, STREET, pos(&st), you(&st)), "{:?}", pos(&st));
        assert!(st.bots[0].wake.len() <= 3);
    }

    #[test]
    fn a_new_city_starts_with_no_bots_out() {
        let mut st = lot();
        sync(&mut st);
        st.bots[0].moved_at = 12345;
        st.enter_city(City::lot(2));
        assert!(st.bots.is_empty(), "the last game's drone stays there");
    }

    #[test]
    fn it_catches_up_on_the_diagonal() {
        let mut st = lot();
        sync(&mut st);
        // Six tiles off on both axes: diagonal steps close that in about five.
        st.player.as_mut().unwrap().pos = Pos::new(16, 14);
        run(&mut st, 3 * 6);
        assert!(with_you(&st, STREET, pos(&st), you(&st)), "{:?}", pos(&st));
    }

    #[test]
    fn it_steps_off_a_foe() {
        let mut st = lot();
        sync(&mut st);
        let at = pos(&st);
        st.foes.push(crate::foes::Foe::goon(STREET, at));
        run(&mut st, 6);
        assert_ne!(pos(&st), at);
        assert!(free(&st, STREET, pos(&st), 0));
    }

    #[test]
    fn it_crosses_places_with_you() {
        let mut st = lot();
        sync(&mut st);
        let sky = Place::Stratum(StratumKind::Skyways);
        {
            let city = st.city.as_mut().unwrap();
            let k = city.stratum_mut(StratumKind::Skyways).unwrap();
            k.tiles.fill_rect(
                citygen::Rect::new(3, 3, 10, 10),
                Tile::new(Terrain::Sidewalk),
            );
        }
        let p = st.player.as_mut().unwrap();
        p.place = sky;
        p.pos = Pos::new(6, 6);
        sync(&mut st);
        assert_eq!(st.bots[0].place, sky);
        assert_eq!(pos(&st).chebyshev(Pos::new(6, 6)), 1);
    }

    #[test]
    fn jacked_in_it_stays_by_your_body() {
        let mut st = lot();
        sync(&mut st);
        let p = st.player.as_mut().unwrap();
        p.body = Some((STREET, Pos::new(10, 8)));
        p.place = Place::Construct(citygen::BuildingId(0));
        p.pos = Pos::new(1, 1);
        run(&mut st, 200);
        assert_eq!(st.bots[0].place, STREET);
        assert!(pos(&st).chebyshev(Pos::new(10, 8)) <= LEASH);
    }

    #[test]
    fn nothing_moves_while_the_clock_stands() {
        let mut st = lot();
        sync(&mut st);
        st.player.as_mut().unwrap().pos = Pos::new(16, 8);
        run(&mut st, 4);
        let (at, wake) = (pos(&st), st.bots[0].wake.clone());
        for _ in 0..100 {
            tick(&mut st);
        }
        assert_eq!((pos(&st), &st.bots[0].wake), (at, &wake));
    }

    #[test]
    fn with_no_free_tile_it_waits() {
        let mut st = lot();
        // Walls all round you, two deep, but one tile, and a foe on that one.
        for dy in -2..=2 {
            for dx in -2..=2 {
                if (dx, dy) != (0, 0) && (dx, dy) != (1, 1) {
                    set(&mut st, Pos::new(10 + dx, 8 + dy), Terrain::Wall);
                }
            }
        }
        st.foes
            .push(crate::foes::Foe::goon(STREET, Pos::new(11, 9)));
        sync(&mut st);
        assert_eq!(st.bots.len(), 1);
        assert_eq!(st.bots[0].pos, None, "out, but nowhere to be");
        st.foes.clear();
        run(&mut st, 1);
        assert_eq!(st.bots[0].pos, Some(Pos::new(11, 9)));
    }

    const DRONE: crate::net::Actor = crate::net::Actor::Bot(game::Site::Back, 0);

    /// On turns on the lot by hand, the drone out where `at` says, your turn ended so
    /// the drone's is next.
    fn on_turns(at: Pos) -> AppState {
        let mut st = lot();
        sync(&mut st);
        st.bots[0].pos = Some(at);
        crate::net::engage(&mut st, STREET, true);
        st
    }

    /// The clock on, a pace each tick, until it is your turn again or `n` ticks pass.
    fn pace_until_you(st: &mut AppState, n: u64) {
        for _ in 0..n {
            if crate::net::phase(st).is_some() {
                return;
            }
            let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
            world.advance(city, world.tick() + 1);
            tick(st);
            crate::net::pace(st);
        }
    }

    #[test]
    fn a_bot_takes_a_seat() {
        let st = on_turns(Pos::new(11, 8));
        let order = &st.net.engaged.as_ref().unwrap().order;
        assert!(order.contains(&DRONE), "{order:?}");
        assert_eq!(order[0], crate::net::Actor::You, "your reflex beats its 6");
    }

    #[test]
    fn on_its_turn_it_comes_back_to_you() {
        let mut st = on_turns(Pos::new(15, 8));
        crate::net::end_turn(&mut st);
        assert_eq!(st.net.engaged.as_ref().unwrap().actor(), DRONE);
        // Off turns it would not move on its own: tick() leaves it be.
        tick(&mut st);
        assert_eq!(pos(&st), Pos::new(15, 8));
        pace_until_you(&mut st, 200);
        assert!(crate::net::phase(&st).is_some(), "your turn again");
        assert!(with_you(&st, STREET, pos(&st), you(&st)), "{:?}", pos(&st));
    }

    #[test]
    fn near_you_it_passes() {
        let mut st = on_turns(Pos::new(11, 8));
        crate::net::end_turn(&mut st);
        assert!(crate::net::phase(&st).is_some(), "its turn passed at once");
        assert_eq!(pos(&st), Pos::new(11, 8));
    }

    #[test]
    fn a_gone_bot_passes_its_seat() {
        let mut st = on_turns(Pos::new(15, 8));
        st.character
            .chrome_copies
            .get_mut(&game::Site::Back)
            .unwrap()
            .fitted[0] = None;
        sync(&mut st);
        crate::net::end_turn(&mut st);
        assert!(crate::net::phase(&st).is_some(), "the empty seat passed");
    }

    #[test]
    fn a_stowed_drone_leaves_the_turn_line() {
        let mut st = on_turns(Pos::new(11, 8));
        st.character
            .chrome_copies
            .get_mut(&game::Site::Back)
            .unwrap()
            .fitted[0] = None;
        sync(&mut st);
        let line = crate::net::turn_readout(&st).unwrap();
        assert!(!line.contains("bot") && !line.contains("drone"), "{line}");
    }

    #[test]
    fn a_bot_hunts_nothing() {
        let st = on_turns(Pos::new(11, 8));
        assert!(!crate::net::hunting(&st), "a drone keeps no fight going");
    }

    #[test]
    fn the_turn_column_shows_the_drone() {
        let mut st = on_turns(Pos::new(15, 8));
        let chips = crate::net::turn_view(&st).unwrap().chips;
        let chip = chips
            .iter()
            .find(|c| c.name == "Scout drone")
            .expect("its chip");
        assert_eq!(chip.hp, "");
        crate::net::end_turn(&mut st);
        let line = crate::net::turn_readout(&st).unwrap();
        assert!(line.contains("Scout drone moves 5"), "{line}");
    }

    #[test]
    fn the_wake_holds_three_beats() {
        let mut st = lot();
        sync(&mut st);
        st.player.as_mut().unwrap().pos = Pos::new(16, 8);
        run(&mut st, 3);
        assert!(!st.bots[0].wake.is_empty(), "a step leaves a wake");
        let p = pos(&st);
        st.player.as_mut().unwrap().pos = Pos::new(p.x - 1, p.y);
        run(&mut st, WAKE_TICKS + 1);
        let now = st.clock_tick();
        assert!(
            st.bots[0].wake.iter().all(|&(_, t)| now < t + WAKE_TICKS),
            "{:?}",
            st.bots[0].wake
        );
    }
}
