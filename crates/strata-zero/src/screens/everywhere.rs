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

/// Steps to try in `area` on `place` round where you stand: one into each kind of thing
/// in the way there (from a cell of the area beside it you can stand on, not a door or a
/// gate, which lead somewhere), and one onto an open cell of it. Each with its kind,
/// `"open"` for the open step.
fn steps(st: &AppState, area: Area, place: Place, reach: i32) -> Vec<(&'static str, Pos, Dir)> {
    let city = st.city.as_ref().unwrap();
    let b = Beyond::of(st).unwrap();
    let k = place.stratum().unwrap();
    let map = MapRef::stratum_at(city, k, st.minute_of_day()).beyond(&b);
    let me = st.player.as_ref().unwrap().pos;
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
    let mut cells: Vec<Pos> = Rect::new(me.x - reach, me.y - reach, 2 * reach + 1, 2 * reach + 1)
        .iter()
        .filter(|q| in_area(st, area, *q))
        .collect();
    cells.sort_by_key(|q| (q.chebyshev(me), q.y, q.x));
    let mut out: Vec<(&'static str, Pos, Dir)> = Vec::new();
    for to in cells {
        let what = if map.walkable(to) {
            if !clear(to) {
                continue;
            }
            "open"
        } else {
            kind(map.tile(to))
        };
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
    out
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
/// open step, what each did.
fn tried(
    st: &mut AppState,
    m: &mut dyn Screen,
    area: Area,
    reach: i32,
) -> Vec<(Place, &'static str, Outcome)> {
    let p = st.player.as_ref().unwrap();
    let (at, body) = (p.pos, p.body.map_or(p.pos, |(_, b)| b));
    let mut out = Vec::new();
    for place in [STREET, NET] {
        stand(st, m, place, at, body);
        let tries = steps(st, area, place, reach);
        for (what, from, d) in tries {
            let outcome = try_step(st, m, place, from, d, body);
            out.push((place, what, outcome));
        }
    }
    // back where you were, on foot
    stand(st, m, STREET, at, body);
    out
}

fn here(st: &AppState) -> Settlement {
    st.outside.as_ref().unwrap().here()
}

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

/// Walked into the next city on foot across the line, by the arrow keys.
fn walked_in() -> (AppState, Box<dyn Screen>, Settlement) {
    let (mut st, mut m) = at_home();
    let (s, x, d) = beside_a_city(&mut st);
    on_foot(&mut st, x);
    m.update(&mut st, &[]);
    m.update(&mut st, &[key(step_key(d))]);
    assert_eq!(here(&st), s, "walked in: {:?}", st.log.latest());
    (st, m, s)
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

/// The home city live, the next city held, and you in its streets held in the home city
/// by a call on the line, so it isn't handed over: the city seen from outside.
fn held_from_outside() -> (AppState, Box<dyn Screen>, Settlement) {
    let (mut st, mut m) = at_home();
    let home = here(&st);
    let (s, _, _) = beside_a_city(&mut st);
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
    (st, m, s)
}

#[test]
fn every_object_blocks_the_same_everywhere() {
    let mut rows: Vec<(&'static str, Place, &'static str, Outcome)> = Vec::new();
    let mut add = |name: &'static str, tried: Vec<(Place, &'static str, Outcome)>| {
        for (place, what, outcome) in tried {
            rows.push((name, place, what, outcome));
        }
    };
    // the home city at the start of a run: what every other place is held to
    let (mut st, mut m) = at_home();
    add("home at start", tried(&mut st, m.as_mut(), Area::Live, 90));
    // walked in across the line
    let (mut st, mut m, s) = walked_in();
    add("walked in", tried(&mut st, m.as_mut(), Area::Live, 90));
    // down from the overworld onto its tile, and that run saved and reopened there
    let (mut st, mut m) = came_down(s);
    add(
        "down from the overworld",
        tried(&mut st, m.as_mut(), Area::Live, 90),
    );
    let mut back = reopen(st, "everywhere-reopened");
    back.rain = false;
    assert_eq!(here(&back), s, "reopened there");
    let mut m = crate::screens::resume_screen(&mut back);
    m.update(&mut back, &[]);
    add("reopened", tried(&mut back, m.as_mut(), Area::Live, 90));
    // home again by ↵ from the overworld
    let (mut st, mut m) = came_home(s);
    add("home again", tried(&mut st, m.as_mut(), Area::Live, 90));
    // your Net self walked in, then jacked out: home again
    let (mut st, mut m, _) = net_and_back();
    add("net and back", tried(&mut st, m.as_mut(), Area::Live, 90));
    // the open ground
    let (mut st, mut m) = on_the_ground();
    add("open ground", tried(&mut st, m.as_mut(), Area::Ground, 40));
    // a city held, seen from outside
    let (mut st, mut m, held) = held_from_outside();
    add(
        "held, from outside",
        tried(&mut st, m.as_mut(), Area::Held(held), 60),
    );

    let table: Vec<String> = rows
        .iter()
        .map(|(n, p, k, o)| format!("{n} · {p:?} · {k} · {o:?}"))
        .collect();
    let table = table.join("\n");
    // what was tried where
    let has = |name: &str, place: Place, what: &str| {
        rows.iter()
            .any(|(n, p, k, _)| *n == name && *p == place && *k == what)
    };
    for name in [
        "home at start",
        "walked in",
        "down from the overworld",
        "reopened",
        "home again",
        "net and back",
    ] {
        for what in ["wall", "terminal", "open"] {
            assert!(has(name, STREET, what), "{name}: no {what} tried\n{table}");
        }
        for what in ["construct wall", "open"] {
            assert!(
                has(name, NET, what),
                "{name}: no {what} on the Net\n{table}"
            );
        }
    }
    for what in ["vending machine", "open"] {
        assert!(
            has("open ground", STREET, what),
            "open ground: {what}\n{table}"
        );
    }
    assert!(has("open ground", NET, "open"), "open Net\n{table}");
    for what in ["wall", "open"] {
        assert!(
            has("held, from outside", STREET, what),
            "held: {what}\n{table}"
        );
    }
    assert!(has("held, from outside", NET, "open"), "held Net\n{table}");
    // the home city's own refusals are the rule
    let home: Vec<_> = rows.iter().filter(|r| r.0 == "home at start").collect();
    for (name, place, what, outcome) in &rows {
        if *what == "open" {
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
