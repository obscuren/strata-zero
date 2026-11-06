//! One set of rules on every map: a step into a wall, a terminal, a vending machine or
//! anything else that stands in the way is refused, and says the same, in the home city,
//! in another however you came into it (walked in across the line, down from the
//! overworld onto its tile, your Net self walked in and jacked out, a run reopened
//! there), on the open ground and in a city held but not live; on foot and on the Net.

use crate::app::{AppState, Screen, ScreenCmd};
use crate::messages::Kind as LogKind;
use crate::outside::stream::tests::handover::{beside_a_city, jacked, on_foot};
use crate::outside::stream::{Beyond, Owner, live_owns, owner_of};
use crate::outside::tests::{home_run, reopen};
use crate::outside::{At, Settlement, TILE};
use crate::render::map::{MapRef, TileMap};
use crate::screens::outside::OutsideScreen;
use crate::screens::testutil::key;
use citygen::{Dir, Feature, Furniture, Place, Pos, Rect, StratumKind, Terrain, Tile};
use crossterm::event::KeyCode;

const STREET: Place = Place::Stratum(StratumKind::Street);
const NET: Place = Place::Stratum(StratumKind::Net);

/// The key that steps `d`.
fn step_key(d: Dir) -> KeyCode {
    KeyCode::Char(match d {
        Dir::N => 'k',
        Dir::S => 'j',
        Dir::E => 'l',
        Dir::W => 'h',
        Dir::NE => 'u',
        Dir::NW => 'y',
        Dir::SE => 'n',
        Dir::SW => 'b',
    })
}

/// What stands in the way on a cell you can't step onto, as the matrix sorts it.
fn kind(t: Option<&Tile>) -> &'static str {
    let Some(t) = t else {
        return "nothing built";
    };
    match (t.feature, t.terrain) {
        (Some(Feature::Terminal), _) => "terminal",
        (Some(Feature::Furniture(Furniture::Vending)), _) => "vending machine",
        (Some(Feature::Furniture(_)), _) => "furniture",
        (Some(Feature::Barrier), _) => "barrier",
        (Some(Feature::Stall), _) => "stall",
        (Some(Feature::AcUnit), _) => "ac unit",
        (Some(Feature::Pipe), _) => "pipe",
        (Some(Feature::Pylon), _) => "pylon",
        (Some(Feature::FireBarrel), _) => "fire barrel",
        (Some(Feature::Ice(_)), _) => "ice",
        (Some(Feature::Sign(_)), _) => "sign",
        (_, Terrain::Wall | Terrain::Window | Terrain::Mass) => "wall",
        (_, Terrain::NetWall) => "construct wall",
        (_, Terrain::Railing) => "railing",
        (_, Terrain::Water | Terrain::Shallows) => "water",
        (_, Terrain::Cliff | Terrain::Rock) => "rock",
        (_, Terrain::Sewage) => "sewage",
        (_, Terrain::Void) => "void",
        _ => "something else",
    }
}

/// What a refused step says, by what's there and nothing else: the ground's words for
/// water, a climb and the world's end, wherever they are; nothing for a wall or anything
/// standing in the way, as in the city; nothing at all on the Net.
fn says(kind: &str, place: Place) -> Vec<String> {
    let line = match kind {
        _ if place == NET => None,
        "water" => Some("too deep to wade"),
        "rock" => Some("too steep to climb"),
        "void" | "nothing built" => Some("the world ends here"),
        _ => None,
    };
    line.into_iter().map(str::to_string).collect()
}

/// Which cells of the live city's frame a situation is about.
#[derive(Clone, Copy, Debug)]
enum Area {
    /// The live city's own.
    Live,
    /// The open ground's: nobody's.
    Ground,
    /// A city or town held, not live.
    Held(Settlement),
}

fn in_area(st: &AppState, area: Area, q: Pos) -> bool {
    match area {
        Area::Live => live_owns(st, q),
        Area::Ground => {
            owner_of(st, q).is_none()
                && Beyond::of(st).is_some_and(|b| matches!(b.elsewhere(q), Some(Owner::Ground(..))))
        }
        Area::Held(s) => !live_owns(st, q) && owner_of(st, q) == Some(s),
    }
}

/// What a step did: whether you moved, and the refusals it logged.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcome {
    moved: bool,
    said: Vec<String>,
}

/// Where to look for steps: every cell of the map an area is on (a city's, live or
/// held), or the cells within so many of you (the open ground, which has no end).
#[derive(Clone, Copy, Debug)]
enum Scope {
    Map,
    Near(i32),
}

/// The cells of the live city's frame in `area`, within `scope`, nearest you first.
fn cells(st: &AppState, area: Area, scope: Scope) -> Vec<Pos> {
    let (city, o) = (st.city.as_ref().unwrap(), st.outside.as_ref().unwrap());
    let me = st.player.as_ref().unwrap().pos;
    let rect = match (scope, area) {
        (Scope::Near(r), _) => Rect::new(me.x - r, me.y - r, 2 * r + 1, 2 * r + 1),
        (Scope::Map, Area::Live) => Rect::new(0, 0, city.size.w as i32, city.size.h as i32),
        (Scope::Map, Area::Held(s)) => {
            let theirs = o.held_city(s).expect("held");
            let (live, at) = (o.origin(city), o.origin(theirs));
            let (w, h) = (theirs.size.w as i32, theirs.size.h as i32);
            Rect::new(at.x - live.x, at.y - live.y, w, h)
        }
        (Scope::Map, Area::Ground) => panic!("the open ground has no end"),
    };
    let mut cells: Vec<Pos> = rect.iter().filter(|q| in_area(st, area, *q)).collect();
    cells.sort_by_key(|q| (q.chebyshev(me), q.y, q.x));
    cells
}

/// Steps to try in `area` on `place` within `scope`: one into each kind of thing in the
/// way there, the nearest you can stand beside on a cell of the area (not a door or a
/// gate, which lead somewhere), and one onto an open cell of it. Each with its kind,
/// `"open"` for the open step. And the kinds there with no cell of the area beside any
/// of them to stand on.
fn steps(
    st: &AppState,
    area: Area,
    place: Place,
    scope: Scope,
) -> (Vec<(&'static str, Pos, Dir)>, Vec<&'static str>) {
    let city = st.city.as_ref().unwrap();
    let b = Beyond::of(st).unwrap();
    let k = place.stratum().unwrap();
    let map = MapRef::stratum_at(city, k, st.minute_of_day()).beyond(&b);
    let world = st.world.as_ref();
    let clear = |q: Pos| world.is_none_or(|w| w.person_at(q).is_none() && w.car_at(q).is_none());
    let stand = |q: Pos| {
        in_area(st, area, q)
            && map.walkable(q)
            && clear(q)
            && !map
                .tile(q)
                .is_some_and(|t| matches!(t.terrain, Terrain::Door | Terrain::NetGate))
    };
    let mut out: Vec<(&'static str, Pos, Dir)> = Vec::new();
    let mut present: Vec<&'static str> = Vec::new();
    for to in cells(st, area, scope) {
        let what = if map.walkable(to) {
            if !clear(to) {
                continue;
            }
            "open"
        } else {
            kind(map.tile(to))
        };
        if !present.contains(&what) {
            present.push(what);
        }
        if out.iter().any(|(k, _, _)| *k == what) {
            continue;
        }
        if let Some(d) = Dir::CARDINAL
            .into_iter()
            .find(|d| stand(to.step(d.opposite())))
        {
            out.push((what, to.step(d.opposite()), d));
        }
    }
    let missing = present
        .into_iter()
        .filter(|k| !out.iter().any(|(t, _, _)| t == k))
        .collect();
    (out, missing)
}

/// You on `place` at `at` (your Net self, jacked in from the street at `body`), the
/// screen caught up with it.
fn stand(st: &mut AppState, m: &mut dyn Screen, place: Place, at: Pos, body: Pos) {
    if place == NET {
        jacked(st, (STREET, body), at);
    } else {
        on_foot(st, at);
    }
    m.update(st, &[]);
}

/// The step `d` from `from` on `place` (your body at `body` on the Net), what it did.
fn try_step(
    st: &mut AppState,
    m: &mut dyn Screen,
    place: Place,
    from: Pos,
    d: Dir,
    body: Pos,
) -> Outcome {
    stand(st, m, place, from, body);
    let p = st.player.as_ref().unwrap();
    assert_eq!((p.place, p.pos), (place, from), "stood where put");
    st.log = Default::default();
    m.update(st, &[key(step_key(d))]);
    let p = st.player.as_ref().unwrap();
    Outcome {
        moved: (p.place, p.pos) != (place, from),
        said: st
            .log
            .iter()
            .filter(|e| e.kind == LogKind::Warn)
            .map(|e| e.text.clone())
            .collect(),
    }
}

/// One situation tried on the street and the Net: each kind of thing in the way and an
/// open step, then a second open step after them all (a key swallowed on the way would
/// show there), what each did; and the kinds with nowhere beside them to stand.
fn tried(st: &mut AppState, m: &mut dyn Screen, area: Area, scope: Scope) -> Tried {
    let p = st.player.as_ref().unwrap();
    let (at, body) = (p.pos, p.body.map_or(p.pos, |(_, b)| b));
    let (mut out, mut missed) = (Vec::new(), Vec::new());
    for place in [STREET, NET] {
        stand(st, m, place, at, body);
        let (tries, missing) = steps(st, area, place, scope);
        missed.extend(missing.into_iter().map(|k| (place, k)));
        let open = tries.iter().find(|(k, _, _)| *k == "open").copied();
        for (what, from, d) in tries {
            let outcome = try_step(st, m, place, from, d, body);
            out.push((place, what, outcome));
        }
        if let Some((_, from, d)) = open {
            let outcome = try_step(st, m, place, from, d, body);
            out.push((place, "open again", outcome));
        }
    }
    // back where you were, on foot
    stand(st, m, STREET, at, body);
    (out, missed)
}

fn here(st: &AppState) -> Settlement {
    st.outside.as_ref().unwrap().here()
}

/// What a situation's tries did, and the kinds it found nowhere to stand beside.
type Tried = (
    Vec<(Place, &'static str, Outcome)>,
    Vec<(Place, &'static str)>,
);

/// The home city at the start of a run, on foot on its street.
fn at_home() -> (AppState, Box<dyn Screen>) {
    let mut st = home_run();
    st.rain = false;
    let mut m: Box<dyn Screen> = Box::new(crate::screens::explore::MapScreen::walking(&mut st));
    m.update(&mut st, &[]);
    (st, m)
}

/// An overworld tile city or town `s` owns, near its own tiles' middle.
fn tile_of(st: &AppState, s: Settlement) -> Pos {
    let o = st.outside.as_ref().unwrap();
    let w = o.world();
    let own = match s.town {
        None => w.city_tiles(s.city),
        Some(t) => w.town_tiles(s.city, t),
    };
    let n = own.len() as i32;
    let mid = own
        .iter()
        .fold(Pos::new(0, 0), |a, p| Pos::new(a.x + p.x, a.y + p.y));
    let mid = Pos::new(mid.x / n.max(1), mid.y / n.max(1));
    own.into_iter()
        .filter(|t| o.owner(*t) == Some(s))
        .min_by_key(|t| t.dist2(mid))
        .expect("a tile it owns")
}

/// City `s` held (built as the game builds it), and a cell of the open ground beside its
/// map (in the live city's frame) with the step from there onto a cell it owns that its
/// street lets you onto; none if it has no such edge.
fn beside(st: &mut AppState, s: Settlement) -> Option<(Pos, Dir)> {
    use citygen::overworld::detail;
    crate::outside::stream::tests::handover::hold(st, s);
    let o = st.outside.as_ref().unwrap();
    let (w, theirs) = (o.world(), o.held_city(s).unwrap());
    let (live, at) = (o.origin(st.city.as_ref().unwrap()), o.origin(theirs));
    let tile = |c: Pos| Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE));
    let (cw, ch) = (theirs.size.w as i32, theirs.size.h as i32);
    (0..ch)
        .flat_map(|y| [(Pos::new(0, y), Dir::E), (Pos::new(cw - 1, y), Dir::W)])
        .chain((0..cw).flat_map(|x| [(Pos::new(x, 0), Dir::S), (Pos::new(x, ch - 1), Dir::N)]))
        .find_map(|(q, d)| {
            let y = at.offset(q.x, q.y);
            let x = y.step(d.opposite());
            let open = w.tile(tile(x)).is_some()
                && detail::region(w, x.x, x.y, 1, 1)[Pos::new(0, 0)].is_walkable();
            (o.owner(tile(y)) == Some(s)
                && o.owner(tile(x)).is_none()
                && theirs.walkable(StratumKind::Street, q)
                && open)
                .then_some((Pos::new(x.x - live.x, x.y - live.y), d))
        })
}

/// The city (not a town) of seed 3's world but the home city with the most kinds of
/// thing in the way on its own street (one with a vending machine first: they're rare
/// out of the home city), that can be walked into from the open ground.
fn richest(st: &AppState) -> Settlement {
    let others: Vec<Settlement> = st
        .outside
        .as_ref()
        .unwrap()
        .others()
        .into_iter()
        .map(|(s, _)| s)
        .filter(|s| s.town.is_none())
        .collect();
    others
        .into_iter()
        .filter_map(|s| {
            let mut st = home_run();
            beside(&mut st, s)?;
            let o = st.outside.as_ref().unwrap();
            let theirs = o.held_city(s).unwrap();
            let at = o.origin(theirs);
            let mut kinds: Vec<&str> = Vec::new();
            for (q, t) in theirs.stratum(StratumKind::Street).unwrap().tiles.iter() {
                let c = at.offset(q.x, q.y);
                let mine = o.owner(Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)));
                let k = kind(Some(t));
                if !t.is_walkable() && mine == Some(s) && !kinds.contains(&k) {
                    kinds.push(k);
                }
            }
            let vending = kinds.contains(&"vending machine");
            Some((vending, kinds.len(), std::cmp::Reverse(s), s))
        })
        .max()
        .expect("a city to walk into")
        .3
}

/// Walked into city `s` on foot across the line from the open ground, by the arrow key.
fn walked_in(s: Settlement) -> (AppState, Box<dyn Screen>) {
    let (mut st, mut m) = at_home();
    let (x, d) = beside(&mut st, s).expect("open ground beside it");
    on_foot(&mut st, x);
    m.update(&mut st, &[]);
    m.update(&mut st, &[key(step_key(d))]);
    assert_eq!(here(&st), s, "walked in: {:?}", st.log.latest());
    (st, m)
}

/// Down from the overworld onto a tile of city `s`, by ↵.
fn came_down(s: Settlement) -> (AppState, Box<dyn Screen>) {
    let mut st = home_run();
    st.rain = false;
    st.outside.as_mut().unwrap().at = At::Overworld(tile_of(&st, s));
    let mut up = OutsideScreen::new(&mut st);
    let ScreenCmd::Switch { to: mut m, .. } = up.update(&mut st, &[key(KeyCode::Enter)]) else {
        panic!("down onto the city screen")
    };
    m.update(&mut st, &[]);
    assert_eq!(here(&st), s, "came down into it");
    (st, m)
}

/// Down into city `s` by ↵, up again from its street by `W`, and down onto a tile of
/// the home city by ↵: home again, set aside and put back.
fn came_home(s: Settlement) -> (AppState, Box<dyn Screen>) {
    let (mut st, mut m) = came_down(s);
    let home = st.outside.as_ref().unwrap().home();
    let ScreenCmd::Switch { to: mut up, .. } = m.update(&mut st, &[key(KeyCode::Char('W'))]) else {
        panic!("up from its street: {:?}", st.log.latest())
    };
    st.outside.as_mut().unwrap().at = At::Overworld(tile_of(&st, home));
    let ScreenCmd::Switch { to: mut m, .. } = up.update(&mut st, &[key(KeyCode::Enter)]) else {
        panic!("down onto the city screen")
    };
    m.update(&mut st, &[]);
    assert_eq!(here(&st), home, "home again");
    (st, m)
}

/// Jacked in on the home city's street, your Net self walked into the next city's Net
/// by the arrow keys, then jacked out (J, and yes): back in the body, home again.
fn net_and_back() -> (AppState, Box<dyn Screen>, Settlement) {
    let (mut st, mut m) = at_home();
    let home = here(&st);
    let body = st.player.as_ref().unwrap().pos;
    let (s, x, d) = beside_a_city(&mut st);
    jacked(&mut st, (STREET, body), x);
    m.update(&mut st, &[]);
    m.update(&mut st, &[key(step_key(d))]);
    assert_eq!(
        here(&st),
        s,
        "your Net self walked in: {:?}",
        st.log.latest()
    );
    let ScreenCmd::Push { to: mut ask, .. } = m.update(&mut st, &[key(KeyCode::Char('J'))]) else {
        panic!("jack out?")
    };
    ask.update(&mut st, &[key(KeyCode::Char('y'))]);
    m.update(&mut st, &[]);
    assert_eq!(here(&st), home, "jacked out: home again");
    let p = st.player.as_ref().unwrap();
    assert_eq!(
        (p.place, p.pos, p.body),
        (STREET, body, None),
        "in the body"
    );
    (st, m, s)
}

/// On the open ground beside a truck stop's pump, on foot.
fn on_the_ground() -> (AppState, Box<dyn Screen>) {
    use citygen::overworld::PoiKind;
    let (mut st, mut m) = at_home();
    let (w, origin) = {
        let o = st.outside.as_ref().unwrap();
        (o.world().clone(), o.origin(st.city.as_ref().unwrap()))
    };
    let poi = w
        .pois
        .iter()
        .find(|p| {
            p.kind == PoiKind::TruckStop
                && (-3..=3).all(|dy| {
                    (-3..=3).all(|dx| !crate::outside::in_any_block(&st, p.at.offset(dx, dy)))
                })
        })
        .expect("a truck stop on the open ground")
        .clone();
    let corner = Pos::new(TILE * (poi.at.x - 3), TILE * (poi.at.y - 3));
    let g = citygen::overworld::detail::region(
        &w,
        corner.x,
        corner.y,
        7 * TILE as u32,
        7 * TILE as u32,
    );
    let pump = g
        .iter()
        .find(|(_, t)| t.feature == Some(Feature::Furniture(Furniture::Vending)))
        .map(|(p, _)| p)
        .expect("its pump");
    let at = Pos::new(
        corner.x + pump.x - origin.x,
        corner.y + pump.y - origin.y + 2,
    );
    let at = crate::outside::nearest(at, 8, |q| {
        let c = Pos::new(q.x + origin.x - corner.x, q.y + origin.y - corner.y);
        g.get(c).is_some_and(|t| t.is_walkable())
    })
    .expect("ground by the pump");
    on_foot(&mut st, at);
    m.update(&mut st, &[]);
    let me = st.player.as_ref().unwrap().pos;
    crate::outside::stream::tend(&mut st, Rect::new(me.x - 48, me.y - 48, 97, 97));
    (st, m)
}

/// The home city live, city `s` held, and you in its streets held in the home city by a
/// call on the line, so it isn't handed over: the city seen from outside.
fn held_from_outside(s: Settlement) -> (AppState, Box<dyn Screen>) {
    let (mut st, mut m) = at_home();
    let home = here(&st);
    crate::outside::stream::tests::handover::hold(&mut st, s);
    ringing(&mut st);
    let inside = {
        let o = st.outside.as_ref().unwrap();
        let theirs = o.held_city(s).unwrap();
        let (live, at) = (o.origin(st.city.as_ref().unwrap()), o.origin(theirs));
        let q = o.street_of(theirs, tile_of(&st, s));
        Pos::new(q.x + at.x - live.x, q.y + at.y - live.y)
    };
    on_foot(&mut st, inside);
    m.update(&mut st, &[]);
    assert_eq!(here(&st), home, "held in: not handed over");
    assert!(in_area(&st, Area::Held(s), inside));
    (st, m)
}

#[test]
fn every_object_blocks_the_same_everywhere() {
    let mut rows: Vec<(String, Place, &'static str, Outcome)> = Vec::new();
    let mut missed: Vec<(String, Place, &'static str)> = Vec::new();
    let mut add = |name: String, (tried, missing): Tried| {
        for (place, what, outcome) in tried {
            rows.push((name.clone(), place, what, outcome));
        }
        for (place, what) in missing {
            missed.push((name.clone(), place, what));
        }
    };
    // the home city at the start of a run: what every other place is held to
    let (mut st, mut m) = at_home();
    let first = beside_a_city(&mut st).0;
    let rich = richest(&st);
    let name = |s: Settlement| st.outside.as_ref().unwrap().name(s);
    let towns = [(first, name(first)), (rich, name(rich))];
    add(
        "home at start".into(),
        tried(&mut st, m.as_mut(), Area::Live, Scope::Map),
    );
    // into the next city on the way out of the home city, and into the one with the most
    // kinds of thing in the way, every way
    for (s, n) in &towns {
        let s = *s;
        let (mut st, mut m) = walked_in(s);
        let live = |what: &str| format!("{what} · {n}");
        add(
            live("walked in"),
            tried(&mut st, m.as_mut(), Area::Live, Scope::Map),
        );
        // down from the overworld onto its tile, and that run saved and reopened there
        let (mut st, mut m) = came_down(s);
        add(
            live("down from the overworld"),
            tried(&mut st, m.as_mut(), Area::Live, Scope::Map),
        );
        let mut back = reopen(st, "everywhere-reopened");
        back.rain = false;
        assert_eq!(here(&back), s, "reopened there");
        let mut m = crate::screens::resume_screen(&mut back);
        m.update(&mut back, &[]);
        add(
            live("reopened"),
            tried(&mut back, m.as_mut(), Area::Live, Scope::Map),
        );
        // home again by ↵ from the overworld
        let (mut st, mut m) = came_home(s);
        add(
            live("home again from"),
            tried(&mut st, m.as_mut(), Area::Live, Scope::Map),
        );
        // held, seen from outside
        let (mut st, mut m) = held_from_outside(s);
        add(
            live("held, from outside"),
            tried(&mut st, m.as_mut(), Area::Held(s), Scope::Map),
        );
    }
    // your Net self walked in, then jacked out: home again
    let (mut st, mut m, _) = net_and_back();
    add(
        "net and back".into(),
        tried(&mut st, m.as_mut(), Area::Live, Scope::Map),
    );
    // the open ground
    let (mut st, mut m) = on_the_ground();
    add(
        "open ground".into(),
        tried(&mut st, m.as_mut(), Area::Ground, Scope::Near(40)),
    );

    let table: Vec<String> = rows
        .iter()
        .map(|(n, p, k, o)| format!("{n} · {p:?} · {k} · {o:?}"))
        .collect();
    let table = table.join("\n");
    // every kind of thing in the way a city's map holds is stepped into, in every
    // situation in it: none with nowhere beside it to stand
    assert!(
        missed.is_empty(),
        "nowhere to stand beside: {missed:?}\n{table}"
    );
    // what was tried where: in every situation an open step before and after the rest,
    // and in each city at least a wall and a terminal on its street and a construct's
    // wall on its Net
    let has = |name: &str, place: Place, what: &str| {
        rows.iter()
            .any(|(n, p, k, _)| n == name && *p == place && *k == what)
    };
    let mut names: Vec<&String> = rows.iter().map(|r| &r.0).collect();
    names.dedup();
    for name in names {
        let mut street = vec!["open", "open again"];
        let mut net = vec!["open", "open again"];
        if name == "open ground" {
            street.extend(["vending machine", "sign"]);
        } else if name.starts_with("held") {
            street.extend(["wall", "terminal"]);
        } else {
            street.extend(["wall", "terminal"]);
            net.push("construct wall");
        }
        for what in street {
            assert!(has(name, STREET, what), "{name}: no {what}\n{table}");
        }
        for what in net {
            assert!(
                has(name, NET, what),
                "{name}: no {what} on the Net\n{table}"
            );
        }
    }
    // the city with the most kinds: its vending machines, furniture and barriers too,
    // every way into it
    let rich = &towns[1].1;
    for way in ["walked in", "down from the overworld", "reopened"] {
        let name = format!("{way} · {rich}");
        for what in ["vending machine", "furniture", "barrier"] {
            assert!(has(&name, STREET, what), "{name}: no {what}\n{table}");
        }
    }
    // the home city's own refusals are the rule
    let home: Vec<_> = rows.iter().filter(|r| r.0 == "home at start").collect();
    for (name, place, what, outcome) in &rows {
        if matches!(*what, "open" | "open again") {
            assert!(
                outcome.moved,
                "{name} · {place:?}: an open step goes\n{table}"
            );
            continue;
        }
        assert!(
            !outcome.moved,
            "{name} · {place:?}: a {what} lets you through\n{table}"
        );
        assert_eq!(
            outcome.said,
            says(what, *place),
            "{name} · {place:?}: a {what} says\n{table}"
        );
        if let Some((_, _, _, at_home)) = home.iter().find(|r| r.1 == *place && r.2 == *what) {
            assert_eq!(
                outcome, at_home,
                "{name} · {place:?}: a {what} as at home\n{table}"
            );
        }
    }
}

/// A sign standing on the ground in `area` within `scope` (a billboard's letters or
/// caps, a road sign: not one on a building's wall) on a tile `which` takes, nearest you,
/// and a cell of the area beside it with the step onto it from there.
fn a_ground_sign(
    st: &AppState,
    area: Area,
    scope: Scope,
    which: impl Fn(&Tile) -> bool,
) -> Option<(Pos, Dir)> {
    let city = st.city.as_ref().unwrap();
    let b = Beyond::of(st).unwrap();
    let map = MapRef::stratum_at(city, StratumKind::Street, st.minute_of_day()).beyond(&b);
    let world = st.world.as_ref();
    let clear = |q: Pos| world.is_none_or(|w| w.person_at(q).is_none() && w.car_at(q).is_none());
    cells(st, area, scope).into_iter().find_map(|to| {
        let t = map.tile(to)?;
        let mut bare = *t;
        bare.feature = None;
        if !matches!(t.feature, Some(Feature::Sign(_))) || !bare.is_walkable() || !which(t) {
            return None;
        }
        Dir::CARDINAL
            .into_iter()
            .map(|d| (to.step(d.opposite()), d))
            .find(|(from, _)| {
                in_area(st, area, *from)
                    && map.walkable(*from)
                    && clear(*from)
                    && map.tile(*from).is_some_and(|t| t.terrain != Terrain::Door)
            })
    })
}

#[test]
fn a_sign_blocks_you() {
    // a billboard on the city's street and a truck stop's sign on the open ground stand
    // in your way as anything else does, and say nothing
    let stopped = Outcome {
        moved: false,
        said: Vec::new(),
    };
    let (mut st, mut m) = at_home();
    // the city's own (a district's), not the world's ground on its map
    let (from, d) = a_ground_sign(&st, Area::Live, Scope::Map, |t| t.district.is_some())
        .expect("a billboard in the home city");
    let street = try_step(&mut st, m.as_mut(), STREET, from, d, from);
    assert_eq!(street, stopped, "a billboard at {:?}", from.step(d));
    let (mut st, mut m) = on_the_ground();
    let (from, d) =
        a_ground_sign(&st, Area::Ground, Scope::Near(40), |_| true).expect("the stop's sign");
    let ground = try_step(&mut st, m.as_mut(), STREET, from, d, from);
    assert_eq!(ground, stopped, "a road sign at {:?}", from.step(d));
}

/// The screen drawn at `w`×`h`, as text, and its footer's rows alone.
fn drawn(st: &AppState, m: &dyn Screen, w: u16, h: u16) -> (String, String) {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| m.draw(st, f)).unwrap();
    let page = crate::screens::testutil::buffer_text(t.backend().buffer());
    let footer = page
        .lines()
        .skip(h as usize - 4)
        .collect::<Vec<_>>()
        .join("\n");
    (page, footer)
}

/// A call ringing, unanswered: what holds you in the live city.
fn ringing(st: &mut AppState) {
    st.missions.call = Some(crate::missions::Call {
        story: None,
        from: "mezz".into(),
        lines: vec!["you there?".into()],
        rang: st.clock_tick(),
        answered: None,
        line: 0,
        shown: 0,
        full: false,
    });
}

/// The middle cell of overworld tile `t`, in the live city's frame.
fn middle(st: &AppState, t: Pos) -> Pos {
    let o = st.outside.as_ref().unwrap();
    let at = o.origin(st.city.as_ref().unwrap());
    Pos::new(TILE * t.x + TILE / 2 - at.x, TILE * t.y + TILE / 2 - at.y)
}

#[test]
fn one_cell_has_one_name() {
    use crate::outside::here::on_the_net;
    let (mut st, mut m) = at_home();
    let body = st.player.as_ref().unwrap().pos;
    let (s, _, _) = beside_a_city(&mut st);
    let live = st.city.as_ref().unwrap().name.clone();
    let (name, trunk, dead) = {
        let o = st.outside.as_ref().unwrap();
        let w = o.world();
        let home = o.block(st.city.as_ref().unwrap());
        // off every settlement's tiles, and well off the home city's map
        let open = |t: &Pos| {
            o.owner(*t).is_none()
                && (t.x - home.x).abs().min((t.x - home.right()).abs()) > 3
                && (t.y - home.y).abs().min((t.y - home.bottom()).abs()) > 3
        };
        let (trunk, road) = w
            .routes
            .iter()
            .flat_map(|r| r.path.iter().map(move |t| (*t, r.name.clone())))
            .find(|(t, _)| open(t))
            .expect("a trunk on the open Net");
        let dead = w
            .tiles
            .iter()
            .map(|(t, _)| t)
            .find(|t| open(t) && !w.routes.iter().any(|r| r.path.contains(t)))
            .expect("dead grid");
        (o.name(s), (trunk, format!("{road} trunk")), dead)
    };
    ringing(&mut st);
    // your Net self over a trunk, over the dead grid, and on the next city's Net (held
    // there by the call): the band, HERE and the minimap say one name, the status bar
    // too, never the live city's
    let theirs = middle(&st, tile_of(&st, s));
    for (at, said) in [
        (middle(&st, trunk.0), trunk.1.clone()),
        (middle(&st, dead), "dead grid".to_string()),
        (theirs, name.clone()),
    ] {
        jacked(&mut st, (STREET, body), at);
        m.update(&mut st, &[]);
        assert_eq!(st.player.as_ref().unwrap().pos, at, "stood there");
        let (page, footer) = drawn(&st, m.as_ref(), 180, 60);
        let named = page.matches(&on_the_net(&said)).count();
        assert!(
            named >= 3,
            "{said}: the band, HERE and the minimap ({named})\n{page}"
        );
        assert!(
            footer.contains(&format!("{said} · grid")),
            "{said}\n{footer}"
        );
        assert!(!page.contains("the open Net"), "{said}\n{page}");
        assert!(
            !footer.contains(&format!("{live} · grid")),
            "{said}\n{footer}"
        );
    }
    // on foot in the next city's streets, held there by the call: HERE names it, and the
    // minimap with it, not the open ground
    on_foot(&mut st, theirs);
    m.update(&mut st, &[]);
    assert_eq!(st.player.as_ref().unwrap().pos, theirs);
    let (page, _) = drawn(&st, m.as_ref(), 180, 60);
    assert!(page.matches(&name).count() >= 3, "{name}\n{page}");
    assert!(!page.contains("open ground"), "{page}");
    // and the run's card, saved here, the same
    assert_eq!(crate::runs::describe::place_line(&st), name);
}

#[test]
fn a_city_is_called_what_the_world_calls_it() {
    use crate::outside::stream::tests::handover::hold;
    // every city and town of seed 3's world, the home city first: live, its own name is
    // the one HERE and the overworld give it, and where the status bar and the top band
    // fall back on it (a Net cell with no district) they say it too
    let base = home_run();
    let o = base.outside.as_ref().unwrap();
    let all: Vec<Settlement> = std::iter::once(o.home())
        .chain(o.others().into_iter().map(|(s, _)| s))
        .collect();
    let mut checked = 0;
    for s in all {
        // down into it from the overworld (built as the game builds it)
        let (mut st, mut m) = at_home();
        if s != here(&st) {
            hold(&mut st, s);
            st.outside.as_mut().unwrap().at = At::Overworld(tile_of(&st, s));
            let mut up = OutsideScreen::new(&mut st);
            let ScreenCmd::Switch { to, .. } = up.update(&mut st, &[key(KeyCode::Enter)]) else {
                panic!("{s:?}: down onto the city screen")
            };
            m = to;
            m.update(&mut st, &[]);
        }
        assert_eq!(here(&st), s, "in it");
        let name = st.outside.as_ref().unwrap().name(s);
        let city = st.city.as_ref().unwrap();
        assert_eq!(city.name, name, "{s:?}: its own name is the world's");
        // a Net cell of its own with no district, beside where you stand
        let me = st.player.as_ref().unwrap().pos;
        let bare = Rect::new(0, 0, city.size.w as i32, city.size.h as i32)
            .iter()
            .filter(|q| {
                live_owns(&st, *q)
                    && city.walkable(StratumKind::Net, *q)
                    && city
                        .tile(StratumKind::Net, *q)
                        .is_some_and(|t| t.district.is_none())
                    && city
                        .tile(StratumKind::Street, *q)
                        .is_some_and(|t| t.district.is_none())
            })
            .min_by_key(|q| q.chebyshev(me));
        let Some(bare) = bare else {
            continue;
        };
        jacked(&mut st, (STREET, me), bare);
        m.update(&mut st, &[]);
        let (page, footer) = drawn(&st, m.as_ref(), 180, 60);
        let band = page.lines().next().unwrap_or_default().to_string();
        assert!(
            footer.contains(&format!("{name} · grid")),
            "{s:?}\n{footer}"
        );
        assert!(band.contains(&format!("{name} · Net")), "{s:?}\n{band}");
        checked += 1;
    }
    assert!(
        checked >= 3,
        "the status bar and the band read in a few: {checked}"
    );
}

/// The screen drawn at `w`×`h`: what it put in each cell.
fn buffer(st: &AppState, m: &dyn Screen, w: u16, h: u16) -> ratatui::buffer::Buffer {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| m.draw(st, f)).unwrap();
    t.backend().buffer().clone()
}

/// The screen cell your `@` is drawn on: the white one on the map (left of the sidebar,
/// whose minimap draws you too).
fn your_cell(buf: &ratatui::buffer::Buffer) -> (u16, u16) {
    let white = ratatui::style::Color::Rgb(255, 255, 255);
    let area = buf.area;
    (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .filter(|at| buf[*at].symbol() == "@" && buf[*at].fg == white)
        .min_by_key(|(x, _)| *x)
        .expect("your @ is drawn")
}

/// The screen drawn at 180×60 and the camera let come to rest on you.
fn settled(st: &mut AppState, m: &mut dyn Screen) {
    buffer(st, m, 180, 60);
    for _ in 0..60 {
        m.update(st, &[]);
    }
}

fn me(st: &AppState) -> Pos {
    st.player.as_ref().unwrap().pos
}

/// In sight from where you stand, as the map draws it: the live map's cells by your
/// sight there, the cells past its edge by what was in sight at the last look; on the
/// Net, all of it.
fn in_sight(st: &AppState, q: Pos) -> bool {
    use crate::render::Visibility;
    let p = st.player.as_ref().unwrap();
    if p.place == NET {
        return true;
    }
    match p.vis().get(q) {
        Some(v) => *v == Visibility::Visible,
        None => p.beyond_sight().binary_search(&q).is_ok(),
    }
}

/// Nobody standing on cell `q` of the live city's frame: none of the live city's people,
/// nor of a city or town held there.
fn nobody(st: &AppState, q: Pos) -> bool {
    let b = Beyond::of(st).unwrap();
    let theirs = b.held().any(|h| {
        let at = Pos::new(
            q.x + b.origin().x - h.origin.x,
            q.y + b.origin().y - h.origin.y,
        );
        h.people
            .is_some_and(|w| w.person_at(at).is_some() || w.car_at(at).is_some())
    });
    let ours = st
        .world
        .as_ref()
        .is_some_and(|w| w.person_at(q).is_some() || w.car_at(q).is_some());
    !theirs && !ours
}

/// Cells of `area` within `r` of you on the map you're on, in sight with nobody on them,
/// one of each kind there (`"open"` for one you could stand on), nearest you first.
fn in_view(st: &AppState, area: Area, r: i32) -> Vec<(&'static str, Pos)> {
    let city = st.city.as_ref().unwrap();
    let b = Beyond::of(st).unwrap();
    let k = st.player.as_ref().unwrap().place.stratum().unwrap();
    let map = MapRef::stratum_at(city, k, st.minute_of_day()).beyond(&b);
    let mut out: Vec<(&'static str, Pos)> = Vec::new();
    for q in cells(st, area, Scope::Near(r)) {
        if q == me(st) || !in_sight(st, q) || !nobody(st, q) || map.tile(q).is_none() {
            continue;
        }
        let what = if map.walkable(q) {
            "open"
        } else {
            kind(map.tile(q))
        };
        if !out.iter().any(|(k, _)| *k == what) {
            out.push((what, q));
        }
    }
    out
}

/// What Look (`x`) says of cell `q` of the map you're on, as the map screen asks it.
fn card_at(st: &AppState, q: Pos) -> crate::look::LookCard {
    let city = st.city.as_ref().unwrap();
    let place = st.player.as_ref().unwrap().place;
    let map = MapRef::stratum_at(city, place.stratum().unwrap(), st.minute_of_day());
    crate::look::look_card(st, &map, place, q, &|_, _| None, &[])
}

/// Look (`x`) with its cursor walked from you onto `q` by the arrow keys, drawn at
/// 180×60, then put away: the character drawn under the cursor, and the page.
fn looked(st: &mut AppState, m: &mut dyn Screen, q: Pos) -> (String, String) {
    // drawn first, so the screen knows its view (the cursor keeps to it)
    settled(st, m);
    let you = your_cell(&buffer(st, m, 180, 60));
    let at = me(st);
    let (dx, dy) = (q.x - at.x, q.y - at.y);
    let mut keys = vec![key(KeyCode::Char('x'))];
    keys.extend(std::iter::repeat_n(
        key(KeyCode::Char(if dx > 0 { 'l' } else { 'h' })),
        dx.unsigned_abs() as usize,
    ));
    keys.extend(std::iter::repeat_n(
        key(KeyCode::Char(if dy > 0 { 'j' } else { 'k' })),
        dy.unsigned_abs() as usize,
    ));
    m.update(st, &keys);
    let buf = buffer(st, m, 180, 60);
    let under = buf[((you.0 as i32 + dx) as u16, (you.1 as i32 + dy) as u16)]
        .symbol()
        .to_string();
    let page = crate::screens::testutil::buffer_text(&buf);
    m.update(st, &[key(KeyCode::Esc)]);
    assert_eq!(me(st), at, "looking never moves you");
    (under, page)
}

const UNSEEN: &str = "You can't see there.";

/// Look on your own cell and on a cell of each kind in sight round you in `area`: each
/// read as it's drawn, the box saying so.
fn reads_as_drawn(st: &mut AppState, m: &mut dyn Screen, area: Area, name: &str) {
    let net = st.player.as_ref().unwrap().place == NET;
    let you = card_at(st, me(st));
    assert_eq!(
        you.header.as_deref(),
        Some("You"),
        "{name}: your own cell {you:?}"
    );
    let (_, page) = looked(st, m, me(st));
    assert!(!page.contains(UNSEEN), "{name}: your own cell\n{page}");
    let cells = in_view(st, area, 12);
    assert!(
        cells.iter().any(|(k, _)| *k == "open"),
        "{name}: an open cell in sight: {cells:?}"
    );
    for (what, q) in cells {
        let card = card_at(st, q);
        assert_ne!(
            card.body,
            vec![UNSEEN.to_string()],
            "{name}: a {what} at {q:?}"
        );
        let header = card
            .header
            .clone()
            .unwrap_or_else(|| panic!("{name}: a {what} named"));
        let (under, page) = looked(st, m, q);
        assert!(page.contains(&header), "{name}: a {what}: {header}\n{page}");
        assert!(!page.contains(UNSEEN), "{name}: a {what} at {q:?}\n{page}");
        // the Net draws its own look over the tiles; on the street the card's glyph is
        // the one drawn
        if !net {
            let glyph = card.glyph.map(|(g, _)| g.to_string());
            assert_eq!(glyph, Some(under), "{name}: a {what} at {q:?}: {card:?}");
        }
    }
}

/// Look on the cells of a city held round you (`area`) and the same cells once it's
/// live (the call put down, and it handed over where you stand): the same cards.
fn as_once_live(st: &mut AppState, m: &mut dyn Screen, s: Settlement, name: &str) {
    let world = |st: &AppState, q: Pos| {
        let o = st.outside.as_ref().unwrap();
        o.origin(st.city.as_ref().unwrap()).offset(q.x, q.y)
    };
    let held: Vec<(&'static str, Pos, crate::look::LookCard)> = in_view(st, Area::Held(s), 10)
        .into_iter()
        .chain(std::iter::once(("you", me(st))))
        .map(|(k, q)| (k, world(st, q), card_at(st, q)))
        .collect();
    st.missions.call = None;
    m.update(st, &[]);
    assert_eq!(here(st), s, "{name}: handed over where you stand");
    for (what, w, card) in held {
        let o = st.outside.as_ref().unwrap();
        let at = o.origin(st.city.as_ref().unwrap());
        let q = Pos::new(w.x - at.x, w.y - at.y);
        assert!(live_owns(st, q));
        assert_eq!(
            card_at(st, q),
            card,
            "{name}: a {what} at {w:?} as once it's live"
        );
    }
}

#[test]
fn look_reads_every_cell_as_drawn() {
    // on foot on the open ground by a truck stop at night
    let (mut st, mut m) = on_the_ground();
    reads_as_drawn(&mut st, m.as_mut(), Area::Ground, "open ground");
    // tab: what's worth a look round you there, the stop's sign and lamps among it
    let pois = {
        let city = st.city.as_ref().unwrap();
        let map = MapRef::stratum_at(city, StratumKind::Street, st.minute_of_day());
        let at = me(&st);
        let view = Rect::new(at.x - 40, at.y - 20, 81, 41);
        crate::look::points_of_interest(&st, &map, STREET, view, at, &|_, _| None)
    };
    let first = *pois
        .first()
        .expect("something worth a look on the open ground");
    settled(&mut st, m.as_mut());
    let you = your_cell(&buffer(&st, m.as_ref(), 180, 60));
    m.update(&mut st, &[key(KeyCode::Char('x')), key(KeyCode::Tab)]);
    let buf = buffer(&st, m.as_ref(), 180, 60);
    let page = crate::screens::testutil::buffer_text(&buf);
    let card = card_at(&st, first);
    let (dx, dy) = (first.x - me(&st).x, first.y - me(&st).y);
    let under = buf[((you.0 as i32 + dx) as u16, (you.1 as i32 + dy) as u16)].symbol();
    assert_eq!(
        card.glyph.map(|(g, _)| g.to_string()).as_deref(),
        Some(under)
    );
    assert!(
        page.contains(card.header.as_deref().unwrap()),
        "tab: {card:?}\n{page}"
    );
    m.update(&mut st, &[key(KeyCode::Esc)]);
    // jacked in there, your Net self on the open Net
    let body = me(&st);
    jacked(&mut st, (STREET, body), body);
    m.update(&mut st, &[]);
    reads_as_drawn(&mut st, m.as_mut(), Area::Ground, "open Net");
    // a city held, from its own streets and its own Net (held in the home city by a
    // call): read as drawn, and as they read once it's live
    let (mut st, _) = at_home();
    let s = beside_a_city(&mut st).0;
    let (mut st, mut m) = held_from_outside(s);
    reads_as_drawn(&mut st, m.as_mut(), Area::Held(s), "held, on foot");
    as_once_live(&mut st, m.as_mut(), s, "held, on foot");
    let (mut st, mut m) = held_from_outside(s);
    let body = me(&st);
    let city = st.city.as_ref().unwrap();
    let b = Beyond::of(&st).unwrap();
    let net = MapRef::stratum_at(city, StratumKind::Net, st.minute_of_day()).beyond(&b);
    let at = crate::outside::nearest(body, 12, |q| {
        in_area(&st, Area::Held(s), q) && net.tile(q).is_some_and(|t| t.terrain == Terrain::NetGrid)
    })
    .expect("its Net's grid");
    drop(b);
    jacked(&mut st, (STREET, body), at);
    m.update(&mut st, &[]);
    assert_eq!(here(&st), here_home(&st), "held in by the call");
    reads_as_drawn(&mut st, m.as_mut(), Area::Held(s), "held, on the Net");
    as_once_live(&mut st, m.as_mut(), s, "held, on the Net");
    // a town's cells on the live city's own map, held in the city by a call
    let mut st = home_run();
    st.rain = false;
    let (_, town, a, d) = crate::outside::stream::tests::handover::beside_a_town(&mut st);
    ringing(&mut st);
    on_foot(&mut st, a.step(d));
    let mut m: Box<dyn Screen> = Box::new(crate::screens::explore::MapScreen::walking(&mut st));
    m.update(&mut st, &[]);
    assert!(
        in_area(&st, Area::Held(town), me(&st)),
        "on the town's cell"
    );
    assert!(
        st.city.as_ref().unwrap().size.contains(me(&st)),
        "on the city's map"
    );
    reads_as_drawn(&mut st, m.as_mut(), Area::Held(town), "a town on the map");
    as_once_live(&mut st, m.as_mut(), town, "a town on the map");
}

fn here_home(st: &AppState) -> Settlement {
    st.outside.as_ref().unwrap().home()
}

#[test]
fn the_free_camera_names_a_cell_as_look_does() {
    use crate::screens::explore::{MapScreen, Mode};
    // dev mode's free camera over the open ground and over a city held: the top band
    // names the cell under its cursor as Look does, never the live city's outskirts
    let (mut st, _) = on_the_ground();
    let ground = me(&st);
    let s = beside_a_city(&mut st).0;
    let theirs = middle(&st, tile_of(&st, s));
    st.unlock_dev();
    for (at, what) in [(ground, "the open ground"), (theirs, "a city held")] {
        let mut m = MapScreen::walking(&mut st);
        m.update(&mut st, &[]);
        m.mode = Mode::Explore;
        m.cursor = at;
        m.update(&mut st, &[]);
        let (page, _) = drawn(&st, &m, 180, 60);
        let band = page.lines().next().unwrap_or_default().to_string();
        let said = crate::look::location(&st, STREET, at);
        assert!(!said.contains("the outskirts"), "{what}: {said}");
        // the band cuts a long name short
        let head: String = said.chars().take(24).collect();
        assert!(band.contains(&head), "{what}: {said}\n{band}");
    }
}
