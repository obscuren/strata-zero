//! The MAP's zoom 4: the world as round 1's survey chart, its places, their cards, and
//! a mark on one of them. Spec `2026-11-04-overworld-screen-design.md`, "The deck MAP's
//! world zoom".

use super::map::MapLayout;
use super::pane::put;
use crate::app::AppState;
use crate::render::overworld::chart_cell;
use crate::ui::chrome::{CYAN, DIM, FAINTER, PANEL_BG};
use citygen::Pos;
use citygen::overworld::{Overworld, RegionKind};
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The hint row at zoom 4.
pub const HINT: &str =
    "←↑↓→ explore · [ ] places · < > kinds · t mark · + the city · f fold · tab close";

/// What zoom 4 remembers: its cursor (a tile of the world) and the kind shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldView {
    pub cursor: Option<Pos>,
    pub kind: Option<WorldKind>,
}

/// The kinds of the world's places PLACES lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldKind {
    City,
    Place,
    Region,
}

impl WorldKind {
    const ALL: [WorldKind; 3] = [WorldKind::City, WorldKind::Place, WorldKind::Region];

    fn label(self) -> &'static str {
        match self {
            WorldKind::City => "cities",
            WorldKind::Place => "places",
            WorldKind::Region => "regions",
        }
    }
}

/// One of the world's places: its name, kind, what it is, and its tile.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub kind: WorldKind,
    pub what: String,
    pub tile: Pos,
}

/// Whether there's a world to zoom out to: a city (not the lot) and its world.
pub fn available(st: &AppState) -> bool {
    st.city.as_ref().is_some_and(|c| !c.is_lot()) && st.outside.is_some()
}

pub use crate::outside::your_tile;

fn region_label(k: RegionKind) -> &'static str {
    match k {
        RegionKind::Range => "mountains",
        RegionKind::Bay => "bay",
        RegionKind::Spill => "dead ground",
        RegionKind::Salt => "salt pan",
        RegionKind::Flats => "flats",
        RegionKind::Valley => "valley",
    }
}

/// The world's cities, places and regions of the kind shown, nearest you first.
pub fn places(st: &AppState) -> Vec<Place> {
    let Some(o) = st.outside.as_ref() else {
        return Vec::new();
    };
    let w = o.world();
    let you = your_tile(st).unwrap_or(Pos::new(0, 0));
    let mut all: Vec<Place> = w
        .cities
        .iter()
        .map(|c| Place {
            name: c.name.clone(),
            kind: WorldKind::City,
            what: if c.home {
                "the city you live in".into()
            } else {
                "a city".into()
            },
            tile: c.centre,
        })
        .chain(w.pois.iter().map(|p| Place {
            name: p.name.clone(),
            kind: WorldKind::Place,
            what: p.kind.label().into(),
            tile: p.at,
        }))
        .chain(w.regions.iter().map(|r| Place {
            name: r.name.clone(),
            kind: WorldKind::Region,
            what: region_label(r.kind).into(),
            tile: r.at,
        }))
        .filter(|p| st.atlas.world.kind.is_none_or(|k| k == p.kind))
        .collect();
    all.sort_by_key(|p| (p.tile.dist2(you), p.name.clone()));
    all
}

/// The place under the cursor: on its tile, or for a city anywhere on its block.
fn under(st: &AppState, all: &[Place]) -> Option<usize> {
    let c = st.atlas.world.cursor?;
    let o = st.outside.as_ref()?;
    all.iter().position(|p| p.tile == c).or_else(|| {
        let s = crate::outside::settlement_at(st, c)?;
        let name = &o.world().cities.get(s.city as usize)?.name;
        all.iter()
            .position(|p| p.kind == WorldKind::City && &p.name == name && s.town.is_none())
    })
}

/// The view's top-left tile: centred on the cursor, held inside the world.
fn origin(world: &Overworld, c: Pos, area: Rect) -> Pos {
    let fit = |at: i32, view: u16, len: u32| {
        let (view, len) = (view as i32, len as i32);
        if len > view {
            (at - view / 2).clamp(0, len - view)
        } else {
            (len - view) / 2
        }
    };
    Pos::new(
        fit(c.x, area.width, world.size.w),
        fit(c.y, area.height, world.size.h),
    )
}

/// Draws zoom 4 into the deck's `body`: the title, the chart, PLACES and the card.
pub fn draw(st: &AppState, body: Rect, buf: &mut Buffer) -> MapLayout {
    let none = MapLayout {
        area: body,
        s: 1,
        m: 1,
        origin: Pos::new(0, 0),
        cursor: None,
        stratum: None,
        pane: 0,
    };
    let Some(o) = st.outside.as_ref() else {
        return none;
    };
    let w = o.world();
    let you = your_tile(st).unwrap_or(Pos::new(0, 0));
    let cursor = st.atlas.world.cursor.unwrap_or(you);
    let pane_w = if st.atlas.folded || body.width < 70 {
        0
    } else {
        34.min(body.width / 3)
    };
    let map = Rect::new(
        body.x,
        body.y + 1,
        body.width.saturating_sub(pane_w),
        body.height.saturating_sub(1),
    );
    let at = origin(w, cursor, map);
    let cell_of = |p: Pos| {
        let (x, y) = (p.x - at.x, p.y - at.y);
        ((0..map.width as i32).contains(&x) && (0..map.height as i32).contains(&y))
            .then(|| (map.x + x as u16, map.y + y as u16))
    };
    for sy in 0..map.height {
        for sx in 0..map.width {
            let p = at.offset(sx as i32, sy as i32);
            chart_cell(w, p, &st.style).write(buf, map.x + sx, map.y + sy);
        }
    }
    let right = map.right();
    let label = |buf: &mut Buffer, p: Pos, text: &str, style: Style, centred: bool| {
        let x0 = if centred {
            p.x - text.chars().count() as i32 / 2
        } else {
            p.x
        };
        for (k, ch) in text.chars().enumerate() {
            if let Some((x, y)) = cell_of(Pos::new(x0 + k as i32, p.y)) {
                buf[(x, y)].set_char(ch).set_style(style);
            }
        }
    };
    // route shields where each highway runs in the open
    let shield = Style::default()
        .fg(Color::Rgb(224, 176, 96))
        .bg(Color::Rgb(26, 20, 8))
        .add_modifier(Modifier::BOLD);
    let mut shown = std::collections::BTreeSet::new();
    for r in &w.routes {
        let Some(num) = r.name.strip_prefix("Route ") else {
            continue;
        };
        if !shown.insert(r.name.clone()) {
            continue;
        }
        let open: Vec<Pos> = r
            .path
            .iter()
            .copied()
            .filter(|p| w.tile(*p).is_some_and(|t| t.urban.is_none()))
            .collect();
        if open.len() >= 10 {
            let p = open[open.len() / 2];
            label(buf, p.offset(1, 0), &format!("[{num}]"), shield, false);
        }
    }
    let dim = |c: Color| Style::default().fg(c);
    for poi in &w.pois {
        let (_, fg) = crate::render::overworld::poi_glyph(poi.kind);
        let fg = fg.mix(crate::render::Rgb(138, 147, 163), 0.45);
        label(
            buf,
            poi.at.offset(2, 0),
            &poi.name,
            dim(fg.to_color()),
            false,
        );
    }
    for r in &w.regions {
        label(buf, r.at, &r.name, dim(Color::Rgb(106, 114, 132)), true);
    }
    let white = Style::default()
        .fg(Color::Rgb(230, 240, 245))
        .add_modifier(Modifier::BOLD);
    for c in &w.cities {
        label(
            buf,
            c.centre.offset(0, -1),
            &c.name.to_uppercase(),
            white,
            true,
        );
    }
    // you, the mark, the cursor
    if let Some((x, y)) = cell_of(you) {
        buf[(x, y)].set_char('@').set_style(white);
    }
    if let Some(m) = o.mark.as_ref()
        && let Some((x, y)) = cell_of(m.tile)
    {
        buf[(x, y)]
            .set_char('⌖')
            .set_style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD));
    }
    // the cursor lit, whatever it's on
    if let Some((x, y)) = cell_of(cursor) {
        buf[(x, y)].set_style(Style::default().fg(Color::Rgb(4, 6, 10)).bg(CYAN));
    }
    // the title: the world, the zoom, the region under the cursor
    let here = w
        .regions
        .iter()
        .min_by_key(|r| r.at.dist2(cursor))
        .map_or(String::new(), |r| format!(" · {}", r.name));
    let title = Style::default()
        .fg(CYAN)
        .bg(PANEL_BG)
        .add_modifier(Modifier::BOLD);
    put(buf, body.x + 1, body.y, "THE WORLD", title, right);
    put(
        buf,
        body.x + 10,
        body.y,
        &format!(" · zoom 4{here}"),
        Style::default().fg(FAINTER).bg(PANEL_BG),
        right,
    );
    if pane_w > 0 {
        pane(st, Rect::new(map.right(), body.y, pane_w, body.height), buf);
    }
    MapLayout {
        area: map,
        s: 1,
        m: 1,
        origin: at,
        cursor: None,
        stratum: None,
        pane: pane_w,
    }
}

/// PLACES: the kinds, the places nearest first (the one under the cursor lit), and its
/// card.
fn pane(st: &AppState, r: Rect, buf: &mut Buffer) {
    let ground = Style::default().bg(PANEL_BG);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_style(ground);
        }
    }
    let (x, right) = (r.x + 1, r.right());
    let head = Style::default()
        .fg(Color::Rgb(160, 170, 185))
        .bg(PANEL_BG)
        .add_modifier(Modifier::BOLD);
    put(buf, x, r.y, "PLACES", head, right);
    let kinds: String = std::iter::once(("all", st.atlas.world.kind.is_none()))
        .chain(
            WorldKind::ALL
                .iter()
                .map(|k| (k.label(), st.atlas.world.kind == Some(*k))),
        )
        .map(|(l, on)| if on { format!("[{l}]") } else { l.to_string() })
        .collect::<Vec<_>>()
        .join(" ");
    put(
        buf,
        x,
        r.y + 1,
        &kinds,
        Style::default().fg(DIM).bg(PANEL_BG),
        right,
    );
    let all = places(st);
    let sel = under(st, &all);
    let you = your_tile(st).unwrap_or(Pos::new(0, 0));
    let card_rows = 5u16;
    let rows = r.height.saturating_sub(3 + card_rows) as usize;
    let first = sel.map_or(0, |s| s.saturating_sub(rows.saturating_sub(1)));
    for (k, p) in all.iter().enumerate().skip(first).take(rows) {
        let y = r.y + 2 + (k - first) as u16;
        let (dx, dy) = (p.tile.x - you.x, p.tile.y - you.y);
        let d = ((dx * dx + dy * dy) as f32).sqrt().round() as i32;
        let way = format!("{d:>3} {:<2}", crate::outside::here::bearing(dx, dy));
        let w = (r.width as usize).saturating_sub(way.len() + 3);
        let name: String = p.name.chars().take(w).collect();
        let style = if Some(k) == sel {
            Style::default().fg(Color::Rgb(4, 6, 10)).bg(CYAN)
        } else {
            Style::default().fg(Color::Rgb(205, 212, 222)).bg(PANEL_BG)
        };
        put(buf, x, y, &format!("{name:<w$}", w = w), style, right);
        put(
            buf,
            right.saturating_sub(way.len() as u16 + 1),
            y,
            &way,
            Style::default().fg(DIM).bg(PANEL_BG),
            right,
        );
    }
    // the card of the place under the cursor
    let Some(p) = sel.map(|s| &all[s]) else {
        return;
    };
    let y = r.bottom().saturating_sub(card_rows);
    put(
        buf,
        x,
        y,
        &"─".repeat(r.width.saturating_sub(2) as usize),
        Style::default().fg(Color::Rgb(40, 46, 56)).bg(PANEL_BG),
        right,
    );
    put(buf, x, y + 1, &p.name, head, right);
    // a city you've been into says so
    let been = p.kind == WorldKind::City
        && st.outside.as_ref().is_some_and(|o| {
            o.world()
                .cities
                .iter()
                .position(|c| c.name == p.name)
                .is_some_and(|i| {
                    o.visited.contains(&crate::outside::Settlement {
                        city: i as u8,
                        town: None,
                    })
                })
        });
    let what = if been {
        format!("{} · you've been", p.what)
    } else {
        p.what.clone()
    };
    put(
        buf,
        x,
        y + 2,
        &what,
        Style::default().fg(DIM).bg(PANEL_BG),
        right,
    );
    let (dx, dy) = (p.tile.x - you.x, p.tile.y - you.y);
    let d = ((dx * dx + dy * dy) as f32).sqrt().round() as i32;
    let way = if d == 0 {
        "you're here".to_string()
    } else {
        format!("{d} tiles {}", crate::outside::here::bearing(dx, dy))
    };
    let road = st
        .outside
        .as_ref()
        .and_then(|o| {
            o.world()
                .routes
                .iter()
                .min_by_key(|r| {
                    r.path
                        .iter()
                        .map(|q| q.dist2(p.tile))
                        .min()
                        .unwrap_or(i64::MAX)
                })
                .map(|r| format!(" · by {}", r.name))
        })
        .unwrap_or_default();
    put(
        buf,
        x,
        y + 3,
        &format!("{way}{road}"),
        Style::default().fg(DIM).bg(PANEL_BG),
        right,
    );
    put(
        buf,
        x,
        y + 4,
        "t mark it",
        Style::default().fg(FAINTER).bg(PANEL_BG),
        right,
    );
}

/// The keys at zoom 4; whether one was taken.
pub fn key(st: &mut AppState, code: KeyCode) -> bool {
    let you = your_tile(st).unwrap_or(Pos::new(0, 0));
    let size = st.outside.as_ref().map(|o| o.world().size);
    let cursor = st.atlas.world.cursor.unwrap_or(you);
    let moved = |dx: i32, dy: i32| {
        let (w, h) = size.map_or((1, 1), |s| (s.w as i32, s.h as i32));
        Pos::new(
            (cursor.x + dx).clamp(0, w - 1),
            (cursor.y + dy).clamp(0, h - 1),
        )
    };
    match code {
        KeyCode::Left | KeyCode::Char('h') => st.atlas.world.cursor = Some(moved(-1, 0)),
        KeyCode::Right | KeyCode::Char('l') => st.atlas.world.cursor = Some(moved(1, 0)),
        KeyCode::Up | KeyCode::Char('k') => st.atlas.world.cursor = Some(moved(0, -1)),
        KeyCode::Down | KeyCode::Char('j') => st.atlas.world.cursor = Some(moved(0, 1)),
        KeyCode::Char('H') => st.atlas.world.cursor = Some(moved(-8, 0)),
        KeyCode::Char('L') => st.atlas.world.cursor = Some(moved(8, 0)),
        KeyCode::Char('K') => st.atlas.world.cursor = Some(moved(0, -4)),
        KeyCode::Char('J') => st.atlas.world.cursor = Some(moved(0, 4)),
        KeyCode::Char('[') | KeyCode::Char(']') => {
            let all = places(st);
            let d: i32 = if code == KeyCode::Char('[') { -1 } else { 1 };
            let next = match under(st, &all) {
                Some(at) => Some((at as i32 + d).rem_euclid(all.len().max(1) as i32) as usize),
                None => (!all.is_empty()).then_some(0),
            };
            if let Some(n) = next {
                st.atlas.world.cursor = Some(all[n].tile);
            }
        }
        KeyCode::Char('<') | KeyCode::Char('>') => {
            let n = WorldKind::ALL.len() as i32 + 1;
            let at = st
                .atlas
                .world
                .kind
                .and_then(|k| WorldKind::ALL.iter().position(|x| *x == k));
            let d = if code == KeyCode::Char('<') { -1 } else { 1 };
            let next = (at.map_or(0, |i| i as i32 + 1) + d).rem_euclid(n);
            st.atlas.world.kind = (next > 0).then(|| WorldKind::ALL[next as usize - 1]);
        }
        KeyCode::Char('t' | 'T' | ' ') => mark(st),
        KeyCode::Char('+' | '=') => {
            st.atlas.zoom = super::Zoom::Fit;
            st.atlas.recentre = true;
        }
        KeyCode::Char('-') => {}
        KeyCode::Char('f' | 'F') => st.atlas.folded = !st.atlas.folded,
        _ => return false,
    }
    true
}

/// `t`: marks the place under the cursor (again on it: the mark goes).
fn mark(st: &mut AppState) {
    let all = places(st);
    let Some(p) = under(st, &all).map(|i| all[i].clone()) else {
        st.warn("nothing here to mark");
        return;
    };
    let Some(o) = st.outside.as_mut() else {
        return;
    };
    if o.mark.as_ref().is_some_and(|m| m.tile == p.tile) {
        o.mark = None;
        st.say(format!("⌖ {} unmarked", p.name));
        return;
    }
    o.mark = Some(crate::outside::Mark {
        tile: p.tile,
        name: p.name.clone(),
    });
    st.say(format!("⌖ marked {}", p.name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::screens::atlas::Zoom;
    use crate::screens::testutil::buffer_text;
    use crossterm::event::KeyCode;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    /// Seed 3's home city on its site, its world made, you on its street.
    fn run() -> AppState {
        let mut st = AppState::new();
        let city = crate::screens::build_home(st.params.clone().with_seed(3)).unwrap();
        st.enter_city(city);
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        crate::outside::ensure(&mut st);
        st
    }

    fn draw(st: &AppState, w: u16, h: u16) -> String {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        crate::screens::atlas::draw(st, area, &mut buf);
        buffer_text(&buf)
    }

    #[test]
    fn minus_past_the_whole_city_is_the_world() {
        let mut st = run();
        st.atlas.zoom = Zoom::Fit;
        draw(&st, 120, 36);
        crate::screens::atlas::key(&mut st, KeyCode::Char('-'));
        assert_eq!(st.atlas.zoom, Zoom::World);
        let text = draw(&st, 120, 36);
        assert!(text.contains("THE WORLD"), "{text}");
        crate::screens::atlas::key(&mut st, KeyCode::Char('+'));
        assert_eq!(st.atlas.zoom, Zoom::Fit, "back to the whole city");
    }

    #[test]
    fn places_are_nearest_first_and_kinds_filter_them() {
        let mut st = run();
        st.atlas.zoom = Zoom::World;
        let all = places(&st);
        let you = your_tile(&st).unwrap();
        let d = |p: &Place| p.tile.dist2(you);
        assert!(
            all.windows(2).all(|w| d(&w[0]) <= d(&w[1])),
            "nearest first"
        );
        assert!(all.iter().any(|p| p.kind == WorldKind::City));
        assert!(all.iter().any(|p| p.kind == WorldKind::Place));
        assert!(all.iter().any(|p| p.kind == WorldKind::Region));
        crate::screens::atlas::key(&mut st, KeyCode::Char('>'));
        let only = places(&st);
        assert!(!only.is_empty() && only.iter().all(|p| Some(p.kind) == st.atlas.world.kind));
    }

    #[test]
    fn t_marks_a_world_place_and_its_way_shows() {
        let mut st = run();
        st.atlas.zoom = Zoom::World;
        let far = places(&st)
            .into_iter()
            .find(|p| p.kind == WorldKind::Place)
            .unwrap();
        st.atlas.world.cursor = Some(far.tile);
        crate::screens::atlas::key(&mut st, KeyCode::Char('t'));
        let mark = st.outside.as_ref().unwrap().mark.clone().expect("marked");
        assert_eq!(
            (mark.tile, mark.name.as_str()),
            (far.tile, far.name.as_str())
        );
        let line = crate::track::ui::pane_line(&st).expect("its way in the pane");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains(&far.name) && text.contains("tiles"), "{text}");
    }

    #[test]
    fn standing_on_the_mark_clears_it() {
        let mut st = run();
        let you = your_tile(&st).unwrap();
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: you.offset(1, 0),
            name: "the spot".into(),
        });
        let o = st.outside.as_mut().unwrap();
        o.at = crate::outside::At::Overworld(you);
        crate::outside::moves::world_step(&mut st, citygen::Dir::E);
        if st.outside.as_ref().unwrap().at == crate::outside::At::Overworld(you.offset(1, 0)) {
            assert!(st.outside.as_ref().unwrap().mark.is_none(), "you're there");
        }
    }

    #[test]
    fn out_in_the_open_map_opens_on_the_world() {
        let mut st = run();
        crate::outside::moves::up(&mut st);
        let mut s = crate::screens::outside::OutsideScreen::new(&mut st);
        crate::app::Screen::update(
            &mut s,
            &mut st,
            &[crate::screens::testutil::key(KeyCode::Tab)],
        );
        assert_eq!(st.atlas.zoom, Zoom::World);
        assert_eq!(st.atlas.world.cursor, None, "on your tile");
    }

    #[test]
    fn a_mark_is_kept_with_the_run() {
        let mut st = run();
        let p = places(&st).into_iter().nth(3).unwrap();
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: p.tile,
            name: p.name.clone(),
        });
        let dir = crate::runs::testutil::Dir::new("mark");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        let m = back.outside.as_ref().unwrap().mark.clone().unwrap();
        assert_eq!((m.tile, m.name), (p.tile, p.name));
    }

    #[test]
    fn the_mark_shows_out_there() {
        let mut st = run();
        let p = places(&st)
            .into_iter()
            .find(|p| p.kind == WorldKind::Place)
            .unwrap();
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: p.tile,
            name: p.name.clone(),
        });
        crate::outside::moves::up(&mut st);
        let s = crate::screens::outside::OutsideScreen::new(&mut st);
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::app::Screen::draw(&s, &st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains(&format!("⌖ {}", p.name)), "{text}");
    }

    #[test]
    fn the_cursor_shows_on_plain_ground() {
        let mut st = run();
        st.atlas.zoom = Zoom::World;
        let w = st.outside.as_ref().unwrap().world().clone();
        let you = your_tile(&st).unwrap();
        let flat = w
            .tiles
            .iter()
            .filter(|(p, t)| {
                t.ground == citygen::overworld::Ground::Flats
                    && t.road == 0
                    && t.urban.is_none()
                    && t.water == citygen::overworld::Water::None
                    && w.poi_at(*p).is_none()
            })
            .map(|(p, _)| p)
            .min_by_key(|p| p.dist2(you))
            .unwrap();
        st.atlas.world.cursor = Some(flat);
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        let l = crate::screens::atlas::draw(&st, area, &mut buf);
        let (x, y) = (
            l.area.x + (flat.x - l.origin.x) as u16,
            l.area.y + (flat.y - l.origin.y) as u16,
        );
        assert_eq!(buf[(x, y)].bg, crate::ui::chrome::CYAN, "the cursor lit");
    }

    #[test]
    fn a_marked_city_clears_on_coming_in() {
        let mut st = run();
        let o = st.outside.as_ref().unwrap();
        let (s, b) = o
            .others()
            .into_iter()
            .find(|(s, b)| s.town.is_none() && b.x > 0)
            .unwrap();
        let c = o.world().cities[s.city as usize].clone();
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: c.centre,
            name: c.name.clone(),
        });
        st.outside.as_mut().unwrap().at =
            crate::outside::At::Overworld(Pos::new(b.x - 1, b.y + b.h / 2));
        let moved = crate::outside::moves::world_step(&mut st, citygen::Dir::E);
        let crate::outside::moves::Moved::City(cell) = moved else {
            panic!("into {}: {moved:?}", c.name);
        };
        crate::outside::moves::into_city(&mut st, cell);
        assert!(
            st.outside.as_ref().unwrap().mark.is_none(),
            "you're in {}",
            c.name
        );
    }

    #[test]
    fn a_place_at_sea_clears_from_its_shore() {
        let mut st = run();
        let w = st.outside.as_ref().unwrap().world().clone();
        let rig = w
            .pois
            .iter()
            .find(|p| p.kind == citygen::overworld::PoiKind::Rig)
            .expect("a rig in seed 3's world")
            .clone();
        let shore = crate::outside::nearest(rig.at, 12, |p| {
            crate::outside::moves::step_cost(&w, p).is_some()
        })
        .unwrap();
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: rig.at,
            name: rig.name.clone(),
        });
        st.outside.as_mut().unwrap().at = crate::outside::At::Overworld(shore);
        crate::outside::reached_mark(&mut st);
        assert!(
            st.outside.as_ref().unwrap().mark.is_none(),
            "as near as you can get"
        );
    }

    #[test]
    fn indoors_the_way_is_from_the_building() {
        let mut st = run();
        let rig = st.home.unwrap();
        let bonus = st.sight_bonus();
        let p = rig
            .wake(
                st.city.as_ref().unwrap(),
                &mut st.interiors,
                &st.constructs,
                bonus,
            )
            .unwrap();
        st.player = Some(p);
        let out = crate::track::listings::walk_from(&st, citygen::StratumKind::Street).unwrap();
        let o = st.outside.as_ref().unwrap();
        assert_eq!(
            your_tile(&st),
            Some(o.tile_of(st.city.as_ref().unwrap(), out))
        );
    }

    #[test]
    fn coming_into_a_city_the_map_is_the_citys() {
        let mut st = run();
        st.atlas.zoom = Zoom::World;
        let cell = st.player.as_ref().unwrap().pos;
        crate::outside::moves::into_city(&mut st, cell);
        assert_ne!(st.atlas.zoom, Zoom::World);
    }

    #[test]
    fn a_mark_leads_the_pane_without_costing_a_job_row() {
        let mut st = run();
        let p = places(&st).into_iter().nth(2).unwrap();
        st.outside.as_mut().unwrap().mark = Some(crate::outside::Mark {
            tile: p.tile,
            name: p.name,
        });
        assert!(st.track.is_none());
        assert_eq!(
            crate::missions::ui::story_line_count(&st),
            1 + crate::missions::ui::jobs_pane(&st).len()
                - crate::jobs::ui::pane_lines(&st).len()
                - 1,
            "the mark's line counted as the track's would be"
        );
    }

    #[test]
    fn the_world_zoom_fits_the_smallest_terminal() {
        let mut st = run();
        st.atlas.zoom = Zoom::World;
        let text = draw(&st, 100, 24);
        assert!(text.contains("THE WORLD"), "{text}");
    }
}
