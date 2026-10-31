//! The map itself: pixels, markers, the cursor, district names and the tracked walk.

use super::{Cache, map_stratum};
use crate::app::AppState;
use crate::render::style::StylePack;
use crate::render::{Rgb, Visibility};
use crate::track::listings::{self, Kind, Listing, source_of};
use crate::track::overlay::{glow, mix, ripple_px};
use crate::track::route::Leg;
use crate::track::{is_tracked, leg_now, target_now};
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, MAGENTA_RGB, PANEL_BG, PANEL_BG_RGB};
use citygen::{
    City, DistrictId, Grid, Place, Pos, Rect as TileRect, Size, StratumKind, Terrain, Tile,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::collections::{BTreeMap, HashSet};

/// The ground of a pixel no district colours.
const NEUTRAL: Rgb = Rgb(40, 44, 52);
const WATER: Rgb = Rgb(14, 40, 72);
const YOU: Color = Color::Rgb(255, 255, 255);
/// How many cells the cursor keeps from the map's edges.
const MARGIN: i32 = 4;

/// The smallest `s` with which a stratum of `size` fits `cols` × `rows` cells, a cell
/// two pixels of `s` × `s` tiles.
pub fn fit_scale(size: Size, cols: u16, rows: u16) -> i32 {
    let up = |n: u32, cells: u32| n.div_ceil(cells.max(1)) as i32;
    up(size.w, cols as u32)
        .max(up(size.h, 2 * rows as u32))
        .max(1)
}

/// Where the map was drawn: the cells, the tiles a pixel covers, the tile at the
/// top-left cell, and the cursor and stratum the draw settled on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapLayout {
    pub area: Rect,
    pub s: i32,
    pub origin: Pos,
    pub cursor: Option<Pos>,
    pub stratum: Option<StratumKind>,
}

impl MapLayout {
    /// The cell a tile falls in, when it is in view.
    pub fn cell_of(&self, tile: Pos) -> Option<(u16, u16)> {
        let s = self.s.max(1);
        let x = (tile.x - self.origin.x).div_euclid(s);
        let y = (tile.y - self.origin.y).div_euclid(2 * s);
        ((0..self.area.width as i32).contains(&x) && (0..self.area.height as i32).contains(&y))
            .then(|| (self.area.x + x as u16, self.area.y + y as u16))
    }

    /// The `s` × `2s` tiles a cell covers.
    pub fn tiles_of(&self, (x, y): (u16, u16)) -> TileRect {
        let s = self.s.max(1);
        TileRect::new(
            self.origin.x + (x as i32 - self.area.x as i32) * s,
            self.origin.y + (y as i32 - self.area.y as i32) * 2 * s,
            s,
            2 * s,
        )
    }

    /// The pixel a tile falls in, counted from the top-left one; may be out of view.
    fn pixel_of(&self, tile: Pos) -> (i32, i32) {
        let s = self.s.max(1);
        (
            (tile.x - self.origin.x).div_euclid(s),
            (tile.y - self.origin.y).div_euclid(s),
        )
    }
}

/// A listing's glyph on the map, or yours: the one that shows on its cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub cell: (u16, u16),
    pub glyph: char,
    pub colour: Color,
    /// Its index in the listings; none for you.
    pub listing: Option<usize>,
}

/// Where you show on stratum `k`'s map: your tile, the door out of the building you
/// are in, or your body's while you are jacked in.
fn you_on(st: &AppState, k: StratumKind) -> Option<Pos> {
    let p = st.player.as_ref()?;
    let on = |place: Place, pos: Pos| match place {
        Place::Stratum(s) => (s == k).then_some(pos),
        Place::Floor { building, level } => st
            .interiors
            .floor(building, level)
            .and_then(|(_, floor)| {
                floor
                    .exits
                    .iter()
                    .filter(|e| e.stratum == k)
                    .min_by_key(|e| (e.at.manhattan(pos), e.at.y, e.at.x))
                    .map(|e| e.door)
            })
            .or_else(|| listings::building_tile(st, building, k)),
        Place::Construct(_) => None,
    };
    on(p.place, p.pos).or_else(|| p.body.and_then(|(place, pos)| on(place, pos)))
}

/// The walk to draw on stratum `k`'s map: the leg while you are on `k`, else the walk
/// from your building's way out.
fn walk_on<'a>(st: &'a AppState, cache: &'a Cache, k: StratumKind) -> Option<&'a Leg> {
    let on_k = st
        .player
        .as_ref()
        .is_some_and(|p| p.place == Place::Stratum(k));
    match leg_now(st) {
        Some(Ok(leg)) if on_k && leg.place == Place::Stratum(k) => Some(leg),
        _ => cache.walk.as_ref(),
    }
}

fn vis_of(st: &AppState, k: StratumKind) -> Option<&Grid<Visibility>> {
    st.player.as_ref()?.vis_at(Place::Stratum(k))
}

/// Walkable ground under whatever stands on it: a stall on a pavement is pavement.
fn ground(t: &Tile) -> bool {
    Tile {
        feature: None,
        ..*t
    }
    .is_walkable()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Building,
    Road,
    Walk,
    Water,
}

/// A pixel's colour from the tiles it covers: its class, its commonest district's
/// theme, and the best you know of them.
fn shade(
    city: &City,
    style: &StylePack,
    k: StratumKind,
    vis: Option<&Grid<Visibility>>,
    x0: i32,
    y0: i32,
    s: i32,
) -> Rgb {
    let Some(stratum) = city.stratum(k) else {
        return PANEL_BG_RGB;
    };
    let (mut n, mut solid, mut water, mut road, mut walk, mut known) = (0, 0, 0, false, false, 0);
    // The districts met, and how often; a pixel meets few.
    let mut districts: [(Option<DistrictId>, u32); 8] = [(None, 0); 8];
    for y in y0..y0 + s {
        for x in x0..x0 + s {
            let p = Pos::new(x, y);
            let Some(t) = stratum.tiles.get(p) else {
                continue;
            };
            n += 1;
            match t.terrain {
                Terrain::Wall
                | Terrain::Mass
                | Terrain::Window
                | Terrain::Rooftop
                | Terrain::Door => solid += 1,
                Terrain::Water | Terrain::Sewage => water += 1,
                Terrain::Road => road = true,
                _ => {}
            }
            walk |= ground(t);
            if let Some(d) = t.district
                && let Some(slot) = districts
                    .iter_mut()
                    .find(|(id, _)| id.is_none_or(|id| id == d))
            {
                *slot = (Some(d), slot.1 + 1);
            }
            known = known.max(match vis.and_then(|g| g.get(p)) {
                Some(Visibility::Visible) => 2,
                Some(Visibility::Remembered) => 1,
                _ => 0,
            });
        }
    }
    let class = if solid * 2 > n {
        Class::Building
    } else if road {
        Class::Road
    } else if walk {
        Class::Walk
    } else if n > 0 && water * 2 >= n {
        Class::Water
    } else {
        return PANEL_BG_RGB;
    };
    // The commonest, the first met on a tie.
    let district = districts
        .iter()
        .fold(
            (None, 0),
            |best, (d, c)| if *c > best.1 { (*d, *c) } else { best },
        )
        .0;
    let theme = district
        .and_then(|d| city.district_opt(d))
        .map(|d| style.theme_or_neutral(Some(d.theme)));
    let colour = match (class, theme) {
        (Class::Water, _) => WATER,
        (_, None) => NEUTRAL,
        (Class::Building, Some(t)) => t.roof,
        (Class::Road, Some(t)) => t.road.scale(1.9),
        (Class::Walk, Some(t)) => t.sidewalk.scale(1.15),
    };
    match known {
        0 if class == Class::Building => PANEL_BG_RGB.mix(colour, 0.22),
        0 => PANEL_BG_RGB.mix(colour, 0.32),
        1 => colour,
        _ => colour.scale(1.3),
    }
}

/// The view's pixels, two to a cell, row by row.
struct Pixels {
    w: i32,
    h: i32,
    v: Vec<Rgb>,
}

impl Pixels {
    fn of(st: &AppState, city: &City, k: StratumKind, l: &MapLayout) -> Pixels {
        let (w, h) = (l.area.width as i32, 2 * l.area.height as i32);
        let vis = vis_of(st, k);
        let v = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (x0, y0) = (l.origin.x + x * l.s, l.origin.y + y * l.s);
                shade(city, &st.style, k, vis, x0, y0, l.s)
            })
            .collect();
        Pixels { w, h, v }
    }

    fn get_mut(&mut self, (x, y): (i32, i32)) -> Option<&mut Rgb> {
        ((0..self.w).contains(&x) && (0..self.h).contains(&y))
            .then(|| &mut self.v[(y * self.w + x) as usize])
    }

    /// The top and bottom pixels of the cell `x`, `y` cells into the map.
    fn cell(&self, x: u16, y: u16) -> (Rgb, Rgb) {
        let at = |py: i32| self.v[(py * self.w + x as i32) as usize];
        (at(2 * y as i32), at(2 * y as i32 + 1))
    }
}

/// The tracked walk on the pixels: each pixel it crosses lit by its first tile in the
/// street's floor glow, then the ripple round the target (or, when the target is not on
/// this map, a dimmer cyan one round the way on).
fn glow_walk(
    st: &AppState,
    px: &mut Pixels,
    l: &MapLayout,
    leg: &Leg,
    k: StratumKind,
    quiet: Option<Rect>,
) {
    let (t, n) = (st.tick, leg.path.len());
    // Nothing moves under the card.
    let still = |(x, y): (i32, i32)| {
        quiet.is_some_and(|r| {
            let (cx, cy) = (l.area.x as i32 + x, l.area.y as i32 + y.div_euclid(2));
            (r.x as i32..r.right() as i32).contains(&cx)
                && (r.y as i32..r.bottom() as i32).contains(&cy)
        })
    };
    let mut lit = HashSet::new();
    for (i, p) in leg.path.iter().enumerate() {
        let at = l.pixel_of(*p);
        if lit.insert(at)
            && !still(at)
            && let Some(c) = px.get_mut(at)
        {
            *c = c.mix(CYAN_RGB, glow(i, n, t));
        }
    }
    let target = target_now(st).and_then(|x| listings::target_tile(st, k, x));
    let (centre, colour, strength) = match target {
        Some(p) => (p, MAGENTA_RGB, 1.0),
        None => (leg.goal, CYAN_RGB, 0.5),
    };
    let (cx, cy) = l.pixel_of(centre);
    for dy in -7..=7 {
        for dx in -7..=7 {
            let f = strength * ripple_px(((dx * dx + dy * dy) as f32).sqrt(), t);
            if f >= 0.02
                && !still((cx + dx, cy + dy))
                && let Some(c) = px.get_mut((cx + dx, cy + dy))
            {
                *c = c.mix(colour, f);
            }
        }
    }
}

/// The pixels as half blocks: `▀` in the top pixel's colour on the bottom's.
fn paint(buf: &mut Buffer, l: &MapLayout, px: &Pixels) {
    for y in 0..l.area.height {
        for x in 0..l.area.width {
            let (top, bottom) = px.cell(x, y);
            let c = &mut buf[(l.area.x + x, l.area.y + y)];
            c.reset();
            c.set_char(if top == bottom { ' ' } else { '▀' })
                .set_fg(top.to_color())
                .set_bg(bottom.to_color());
        }
    }
}

/// Whether a listing shows, on the map and in the pane: of the chosen kind, pinned, or
/// tracked.
pub fn shows(st: &AppState, l: &Listing) -> bool {
    st.atlas.kind.is_none_or(|k| k == l.kind) || l.kind.pinned() || tracked(st, l)
}

fn tracked(st: &AppState, l: &Listing) -> bool {
    is_tracked(st, &source_of(st, l))
}

/// Which listing's marker wins a shared cell, lowest first: the tracked one, then jobs
/// and the story, then home, then the rest (you are 0).
pub fn rank(st: &AppState, l: &Listing) -> u8 {
    match l.kind {
        _ if tracked(st, l) => 1,
        Kind::Jobs => 2,
        Kind::Home => 3,
        _ => 4,
    }
}

/// The markers on the map, one a cell: you, then the tracked one, then the pinned
/// (jobs and story, home), then the rest, the nearest first. Listings of the chosen
/// kind show, and the pinned and the tracked one whatever the kind.
pub fn markers(st: &AppState, layout: &MapLayout, listings: &[Listing]) -> Vec<Marker> {
    let mut best: BTreeMap<(u16, u16), (u8, Marker)> = BTreeMap::new();
    let mut offer = |rank: u8, m: Marker| {
        let e = best.entry(m.cell).or_insert((rank, m));
        if rank < e.0 {
            *e = (rank, m);
        }
    };
    for (i, l) in listings.iter().enumerate() {
        let Some(cell) = layout.cell_of(l.tile).filter(|_| shows(st, l)) else {
            continue;
        };
        let m = Marker {
            cell,
            glyph: l.kind.glyph(),
            colour: l.kind.color(),
            listing: Some(i),
        };
        offer(rank(st, l), m);
    }
    let you = layout.stratum.and_then(|k| you_on(st, k));
    if let Some(cell) = you.and_then(|p| layout.cell_of(p)) {
        let m = Marker {
            cell,
            glyph: '@',
            colour: YOU,
            listing: None,
        };
        offer(0, m);
    }
    best.into_values().map(|(_, m)| m).collect()
}

/// The cursor's four corners round its cell, bold cyan, the cell lit a little; the
/// cells it took.
fn draw_cursor(buf: &mut Buffer, area: Rect, cell: (u16, u16)) -> Vec<(u16, u16)> {
    let mut took = vec![cell];
    for (dx, dy, ch) in [(-1, -1, '┌'), (1, -1, '┐'), (-1, 1, '└'), (1, 1, '┘')] {
        let (x, y) = (cell.0 as i32 + dx, cell.1 as i32 + dy);
        let inside = (area.x as i32..area.right() as i32).contains(&x)
            && (area.y as i32..area.bottom() as i32).contains(&y);
        if !inside {
            continue;
        }
        let at = (x as u16, y as u16);
        let c = &mut buf[at];
        c.set_char(ch).set_fg(CYAN);
        c.modifier.insert(Modifier::BOLD);
        took.push(at);
    }
    let c = &mut buf[cell];
    c.set_bg(mix(c.bg, CYAN, 0.25));
    took
}

/// Each district's name at its centre in its accent, on the nearest row where it
/// covers nothing `taken` (a cell spare either side); left out when none fits.
fn draw_names(
    buf: &mut Buffer,
    st: &AppState,
    city: &City,
    l: &MapLayout,
    taken: &mut HashSet<(u16, u16)>,
) {
    let area = l.area;
    for d in &city.districts {
        let Some((cx, cy)) = l.cell_of(d.center) else {
            continue;
        };
        let name: Vec<char> = d.name.to_uppercase().chars().collect();
        let len = name.len() as i32;
        if len == 0 || len > area.width as i32 {
            continue;
        }
        let x = (cx as i32 - len / 2).clamp(area.x as i32, area.right() as i32 - len);
        let free = |y: &i32| {
            (area.y as i32..area.bottom() as i32).contains(y)
                && (x - 1..=x + len).all(|k| k < 0 || !taken.contains(&(k as u16, *y as u16)))
        };
        let rows = [0, 1, -1, 2, -2, 3, -3].map(|dy| cy as i32 + dy);
        let Some(y) = rows.into_iter().find(free) else {
            continue;
        };
        let accent = st.style.theme_or_neutral(Some(d.theme)).accent;
        let fg = accent.scale(0.55).to_color();
        for (i, ch) in name.iter().enumerate() {
            let at = ((x + i as i32) as u16, y as u16);
            buf[at].set_char(*ch).set_fg(fg);
            taken.insert(at);
        }
    }
}

/// The first tile of the view along one axis, `cells` cells of `per` tiles over a
/// stratum `len` long: centred when the stratum is shorter than the view; else panned
/// to keep the cursor `c` four cells inside (centred on it when it starts or leaves the
/// view) and clamped to the stratum. Always on the stratum's grid of cells, so a cell
/// covers the same tiles however the view got there.
fn pan(prev: i32, c: i32, len: i32, cells: i32, per: i32, restart: bool) -> i32 {
    if len <= cells * per {
        let used = (len + per - 1) / per;
        return -((cells - used) / 2) * per;
    }
    let margin = MARGIN.min((cells - 1) / 2);
    let at = (c - prev).div_euclid(per);
    let o = if restart || !(0..cells).contains(&at) {
        c - cells / 2 * per
    } else if at < margin {
        c - margin * per
    } else if at > cells - 1 - margin {
        c - (cells - 1 - margin) * per
    } else {
        prev
    };
    let grid = |o: i32| o - o.rem_euclid(per);
    let far = len - cells * per;
    grid(o).clamp(0, far + (per - far.rem_euclid(per)) % per)
}

/// Where the map in `area` looks and where its cursor stands (spec 7): the cursor
/// starts on the tracked target, else on you, when the map's stratum is new to it or it
/// has none; the view pans to keep it four cells inside, or centres on it when it
/// starts or after a zoom.
pub fn layout(st: &AppState, area: Rect) -> MapLayout {
    let mut layout = MapLayout {
        area,
        s: 1,
        origin: Pos::default(),
        cursor: None,
        stratum: None,
    };
    let k = map_stratum(st);
    let size = st
        .city
        .as_ref()
        .and_then(|c| c.stratum(k))
        .map(|s| s.tiles.size())
        .filter(|s| s.w > 0 && s.h > 0);
    let (false, Some(size), Some(_)) = (area.is_empty(), size, st.player.as_ref()) else {
        return layout;
    };
    let s = st
        .atlas
        .zoom
        .tiles(fit_scale(size, area.width, area.height));
    let restart = st.atlas.stratum != Some(k) || st.atlas.cursor.is_none();
    let start = || {
        target_now(st)
            .and_then(|t| listings::target_tile(st, k, t))
            .or_else(|| you_on(st, k))
            .unwrap_or(size.center())
    };
    let c = match st.atlas.cursor {
        Some(c) if !restart => c,
        _ => start(),
    };
    let (w, h) = (size.w as i32, size.h as i32);
    let cursor = Pos::new(c.x.clamp(0, w - 1), c.y.clamp(0, h - 1));
    let (prev, cols, rows) = (st.atlas.origin, area.width as i32, area.height as i32);
    let centre = restart || st.atlas.recentre;
    layout.s = s;
    layout.origin = Pos::new(
        pan(prev.x, cursor.x, w, cols, s, centre),
        pan(prev.y, cursor.y, h, rows, 2 * s, centre),
    );
    layout.cursor = Some(cursor);
    layout.stratum = Some(k);
    layout
}

/// Draws the map as `l` lays it out: pixels, the tracked walk on them, markers, the
/// cursor and district names; nothing animates in `quiet` (the card's cells).
pub(super) fn draw_at(
    st: &AppState,
    l: &MapLayout,
    cache: &Cache,
    quiet: Option<Rect>,
    buf: &mut Buffer,
) {
    let area = l.area;
    if area.is_empty() {
        return;
    }
    let (Some(city), Some(k), Some(cursor)) = (st.city.as_ref(), l.stratum, l.cursor) else {
        let ground = Style::default().fg(DIM).bg(PANEL_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(ground);
            }
        }
        let room = area.width.saturating_sub(1) as usize;
        let text = "no signal — walk the city first";
        buf.set_stringn(area.x + 1, area.y, text, room, ground);
        return;
    };
    let mut px = Pixels::of(st, city, k, l);
    if let Some(leg) = walk_on(st, cache, k) {
        glow_walk(st, &mut px, l, leg, k, quiet);
    }
    paint(buf, l, &px);
    let mut taken = HashSet::new();
    for m in markers(st, l, &cache.listings) {
        let (top, bottom) = px.cell(m.cell.0 - area.x, m.cell.1 - area.y);
        let bg = top.mix(bottom, 0.5).scale(0.45);
        let style = Style::default()
            .fg(m.colour)
            .bg(bg.to_color())
            .add_modifier(Modifier::BOLD);
        buf[m.cell].set_char(m.glyph).set_style(style);
        taken.insert(m.cell);
    }
    if let Some(cell) = l.cell_of(cursor) {
        taken.extend(draw_cursor(buf, area, cell));
    }
    draw_names(buf, st, city, l, &mut taken);
}

#[cfg(test)]
mod tests {
    use super::super::tests::{ON_STREET, STREET, outside, put, you};
    use super::super::{Zoom, cached};
    use super::*;
    use crate::track::Source;
    use crate::track::listings::listings;
    use citygen::City;
    use ratatui::buffer::Cell;
    use std::collections::HashSet;

    /// The map drawn into `area` of a buffer a little larger.
    fn draw(st: &AppState, area: Rect) -> (Buffer, MapLayout) {
        let mut buf = Buffer::empty(Rect::new(0, 0, area.right() + 3, area.bottom() + 2));
        let l = layout(st, area);
        draw_at(st, &l, &cached(st), None, &mut buf);
        (buf, l)
    }

    /// The colour of the `s` × `s` pixel at tile (`x0`, `y0`) of a stratum (spec 7).
    fn pixel(st: &AppState, stratum: StratumKind, x0: i32, y0: i32, s: i32) -> Color {
        match st.city.as_ref() {
            Some(city) => {
                shade(city, &st.style, stratum, vis_of(st, stratum), x0, y0, s).to_color()
            }
            None => PANEL_BG,
        }
    }

    /// The nearest ripperdoc on the street that a walk reaches.
    fn clinic(st: &AppState) -> Listing {
        listings(st, STREET)
            .into_iter()
            .find(|l| l.kind == Kind::Docs && l.walk.is_some())
            .expect("a ripperdoc on the street")
    }

    #[test]
    fn no_city_no_map() {
        let st = AppState::new();
        let (buf, l) = draw(&st, Rect::new(0, 0, 40, 3));
        assert_eq!((l.cursor, l.stratum), (None, None));
        let row: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(row.contains("no signal"), "{row}");
        // A sliver of a map never panics.
        let st = outside();
        for (w, h) in [(0, 0), (1, 1), (2, 1), (1, 3), (3, 3)] {
            draw(&st, Rect::new(0, 0, w, h));
        }
    }

    #[test]
    fn fit_scale_fits() {
        let city = Size { w: 240, h: 120 };
        assert_eq!(fit_scale(city, 81, 26), 3);
        assert_eq!(fit_scale(city, 66, 21), 4);
        assert_eq!(fit_scale(Size { w: 30, h: 10 }, 81, 26), 1);
        assert!(fit_scale(city, 0, 0) >= 1, "no cells never divides by zero");
        assert_eq!(Zoom::default(), Zoom::Two);
        assert_eq!(Zoom::One.tiles(3), 1);
        assert_eq!(Zoom::Two.tiles(3), 2);
        assert_eq!(Zoom::Fit.tiles(3), 3);
        // Fit is never further out than the whole city needs, and 2 never past fit.
        assert_eq!(Zoom::Two.tiles(1), 1);
        assert_eq!(Zoom::Fit.tiles(1), 1);
    }

    /// The first 2 × 2 block of street tiles of one district that `f` accepts and you
    /// have never seen.
    fn block(st: &AppState, f: impl Fn(&Tile) -> bool) -> Pos {
        let city = st.city.as_ref().unwrap();
        let vis = st.player.as_ref().unwrap().vis_at(ON_STREET);
        let unseen = |p: Pos| {
            vis.and_then(|g| g.get(p))
                .is_none_or(|v| *v == Visibility::Unseen)
        };
        let size = city.size;
        (0..size.h as i32 - 1)
            .flat_map(|y| (0..size.w as i32 - 1).map(move |x| Pos::new(x, y)))
            .find(|p| {
                let ps = [*p, p.offset(1, 0), p.offset(0, 1), p.offset(1, 1)];
                let first = city.tile(STREET, *p).unwrap().district;
                ps.iter().all(|q| {
                    let t = city.tile(STREET, *q).unwrap();
                    f(t) && t.district == first && unseen(*q)
                })
            })
            .expect("a block of those tiles")
    }

    fn remember(st: &mut AppState, at: Pos) {
        let size = st.city.as_ref().unwrap().size;
        let ps = [at, at.offset(1, 0), at.offset(0, 1), at.offset(1, 1)];
        let p = st.player.as_mut().unwrap();
        p.remember_place(ON_STREET, size, ps.into_iter());
    }

    #[test]
    fn pixels_class_by_their_tiles() {
        let mut st = outside();
        let theme = |st: &AppState, p: Pos| {
            let city = st.city.as_ref().unwrap();
            let d = city.tile(STREET, p).unwrap().district.unwrap();
            st.style
                .theme_or_neutral(Some(city.district(d).theme))
                .clone()
        };
        let px = |st: &AppState, p: Pos| pixel(st, STREET, p.x, p.y, 2);

        let mass = block(&st, |t| t.terrain == Terrain::Mass && t.district.is_some());
        let roof = theme(&st, mass).roof;
        assert_eq!(px(&st, mass), PANEL_BG_RGB.mix(roof, 0.22).to_color());
        remember(&mut st, mass);
        assert_eq!(px(&st, mass), roof.to_color());
        st.player
            .as_mut()
            .unwrap()
            .set_vis(mass.offset(1, 1), Visibility::Visible);
        assert_eq!(px(&st, mass), roof.scale(1.3).to_color());

        let road = block(&st, |t| t.terrain == Terrain::Road && t.district.is_some());
        let lit = theme(&st, road).road.scale(1.9);
        assert_eq!(px(&st, road), PANEL_BG_RGB.mix(lit, 0.32).to_color());
        remember(&mut st, road);
        assert_eq!(px(&st, road), lit.to_color());

        let walk = block(&st, |t| {
            t.terrain == Terrain::Sidewalk && t.district.is_some()
        });
        remember(&mut st, walk);
        let lit = theme(&st, walk).sidewalk.scale(1.15);
        assert_eq!(px(&st, walk), lit.to_color());
        // Stalls stand on the pavement; it is still pavement.
        let city = st.city.as_mut().unwrap();
        for p in [
            walk,
            walk.offset(1, 0),
            walk.offset(0, 1),
            walk.offset(1, 1),
        ] {
            city.tile_mut(STREET, p).unwrap().feature = Some(citygen::Feature::Stall);
        }
        assert_eq!(px(&st, walk), lit.to_color());

        let water = block(&st, |t| t.terrain == Terrain::Water);
        remember(&mut st, water);
        assert_eq!(px(&st, water), Color::Rgb(14, 40, 72));

        // Off the stratum: nothing, the deck's background.
        assert_eq!(pixel(&st, STREET, -10, -10, 2), PANEL_BG);
        let size = st.city.as_ref().unwrap().size;
        assert_eq!(pixel(&st, STREET, size.w as i32, 0, 3), PANEL_BG);
    }

    #[test]
    fn the_cursor_starts_on_you_or_the_track() {
        let mut st = outside();
        let area = Rect::new(1, 1, 81, 26);
        let (_, l) = draw(&st, area);
        assert_eq!(l.cursor, Some(you(&st)));
        assert_eq!(l.stratum, Some(STREET));
        assert!(l.cell_of(you(&st)).is_some());

        let doc = clinic(&st);
        crate::track::set(&mut st, Source::Listing(doc.id)).unwrap();
        let (_, l) = draw(&st, area);
        assert_eq!(l.cursor, Some(doc.tile));
        assert!(l.cell_of(doc.tile).is_some(), "the view pans to it");

        // Settled, it stays where it was, tracked or not.
        st.atlas.settle(&l);
        st.track = None;
        let (_, again) = draw(&st, area);
        assert_eq!(again.cursor, Some(doc.tile));
        assert_eq!(again.origin, l.origin);
        // Another stratum starts it again.
        st.atlas.stratum = Some(StratumKind::Skyways);
        let (_, l) = draw(&st, area);
        assert_eq!(l.cursor, Some(you(&st)));
    }

    #[test]
    fn the_view_clamps_and_centres() {
        let mut st = outside();
        let area = Rect::new(0, 0, 81, 26);
        let size = st.city.as_ref().unwrap().size;
        let (w, h) = (size.w as i32, size.h as i32);
        let at = |st: &mut AppState, p: Pos| {
            st.atlas.stratum = Some(STREET);
            st.atlas.cursor = Some(p);
            draw(st, area).1
        };
        let l = at(&mut st, Pos::new(0, 0));
        assert_eq!((l.s, l.origin), (2, Pos::new(0, 0)));
        let l = at(&mut st, Pos::new(w - 1, h - 1));
        assert_eq!(l.origin, Pos::new(w - 81 * 2, h - 26 * 4));
        // Off the stratum, it is put back on it.
        let l = at(&mut st, Pos::new(-5, 9999));
        assert_eq!(l.cursor, Some(Pos::new(0, h - 1)));

        // Walking right from the middle, the view keeps it four cells inside.
        let mut l = at(&mut st, Pos::new(w / 2, h / 2));
        st.atlas.settle(&l);
        let far = w - 81 * 2;
        for _ in 0..60 {
            let c = st.atlas.cursor.unwrap();
            st.atlas.cursor = Some(c.offset(l.s, 0));
            l = draw(&st, area).1;
            st.atlas.settle(&l);
            let (x, _) = l.cell_of(l.cursor.unwrap()).unwrap();
            if l.origin.x < far {
                assert!((4..=81 - 5).contains(&x), "cell {x} origin {:?}", l.origin);
            }
        }
        assert_eq!(l.origin.x, far, "clamped at the edge");

        // A lot smaller than the map sits in its middle, every tile on a cell.
        let mut st = AppState::new();
        st.enter_city(City::lot(1));
        st.player = Some(crate::walker::Player::at(ON_STREET, Pos::new(10, 8)));
        st.atlas.zoom = Zoom::Fit;
        let (_, l) = draw(&st, area);
        let size = st.city.as_ref().unwrap().size;
        assert_eq!(l.s, 1);
        for p in size.rect().iter() {
            assert!(l.cell_of(p).is_some(), "{p:?}");
        }
        let first = l.cell_of(Pos::new(0, 0)).unwrap();
        let last = l
            .cell_of(Pos::new(size.w as i32 - 1, size.h as i32 - 1))
            .unwrap();
        let (left, right) = (first.0 - area.x, area.right() - 1 - last.0);
        let (top, bottom) = (first.1 - area.y, area.bottom() - 1 - last.1);
        assert!(left > 0 && left.abs_diff(right) <= 1, "{left} {right}");
        assert!(top > 0 && top.abs_diff(bottom) <= 1, "{top} {bottom}");
    }

    #[test]
    fn markers_stack_by_priority() {
        let mut st = outside();
        // A cell of 20 × 40 tiles: home's door, you beside it, and others share it.
        st.atlas.zoom = Zoom::Fit;
        let area = Rect::new(0, 0, 12, 6);
        let all = listings(&st, STREET);
        let home = all.iter().find(|l| l.kind == Kind::Home).unwrap().clone();
        let l = draw(&st, area).1;
        assert_eq!(l.s, 20);
        let cell = l.cell_of(home.tile).unwrap();
        assert_eq!(l.cell_of(you(&st)), Some(cell), "you stand by home's door");
        let rest = all
            .iter()
            .find(|x| !x.kind.pinned() && x.walk.is_some() && l.cell_of(x.tile) == Some(cell))
            .expect("a place near home")
            .clone();
        let glyph = |st: &AppState| {
            let (buf, l) = draw(st, area);
            let c = &buf[l.cell_of(home.tile).unwrap()];
            (c.symbol().to_string(), c.fg, c.modifier)
        };

        crate::track::set(&mut st, Source::Listing(rest.id)).unwrap();
        let (at, fg, bold) = glyph(&st);
        assert_eq!(at, "@");
        assert_eq!(fg, Color::Rgb(255, 255, 255));
        assert!(bold.contains(Modifier::BOLD));

        // You walk off, clear of the cursor's corners: the tracked one shows over home.
        let apart = |c: (u16, u16)| c.0.abs_diff(cell.0).max(c.1.abs_diff(cell.1)) >= 2;
        let away = all
            .iter()
            .map(|x| x.tile)
            .find(|p| l.cell_of(*p).is_some_and(apart))
            .unwrap();
        put(&mut st, ON_STREET, away);
        assert_eq!(glyph(&st).0, rest.kind.glyph().to_string());

        // Untracked: the pinned over the rest, whatever kind is chosen.
        st.track = None;
        let pinned = |s: &str| ["◆", "⌂"].contains(&s);
        assert!(pinned(&glyph(&st).0), "{}", glyph(&st).0);
        st.atlas.kind = Some(Kind::Hotels);
        assert!(pinned(&glyph(&st).0));

        // Without a home, the rest (or a job there, pinned).
        st.atlas.kind = None;
        st.home = None;
        let here: Vec<Kind> = listings(&st, STREET)
            .iter()
            .filter(|x| l.cell_of(x.tile) == Some(cell))
            .map(|x| x.kind)
            .collect();
        let shows = glyph(&st).0;
        if here.contains(&Kind::Jobs) {
            assert_eq!(shows, "◆");
        } else {
            assert!(
                here.iter().any(|k| k.glyph().to_string() == shows),
                "{shows}"
            );
            assert_ne!(shows, "⌂");
        }
        // One marker a cell, and its listing is the one that shows.
        let all = listings(&st, STREET);
        let marks = markers(&st, &l, &all);
        let cells: HashSet<(u16, u16)> = marks.iter().map(|m| m.cell).collect();
        assert_eq!(cells.len(), marks.len());
        for m in marks.iter().filter(|m| m.listing.is_some()) {
            assert_eq!(all[m.listing.unwrap()].kind.glyph(), m.glyph);
        }
    }

    #[test]
    fn the_walk_draws_on_pixels() {
        let mut st = outside();
        let area = Rect::new(2, 1, 81, 26);
        st.atlas.stratum = Some(STREET);
        st.atlas.cursor = Some(you(&st));
        let (before, _) = draw(&st, area);
        let doc = clinic(&st);
        crate::track::set(&mut st, Source::Listing(doc.id)).unwrap();
        let (after, l) = draw(&st, area);
        let leg = crate::track::leg_now(&st).unwrap().unwrap();
        let walked: HashSet<(u16, u16)> = leg.path.iter().filter_map(|p| l.cell_of(*p)).collect();
        assert!(!walked.is_empty());
        assert!(
            walked.iter().any(|c| before[*c] != after[*c]),
            "the walk glows"
        );
        for y in 0..after.area.height {
            for x in 0..after.area.width {
                if !area.contains((x, y).into()) {
                    assert_eq!(after[(x, y)], Cell::default(), "at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn the_walk_starts_at_your_way_out() {
        let mut st = outside();
        let doc = clinic(&st);
        crate::track::set(&mut st, Source::Listing(doc.id)).unwrap();
        // Inside, by the megablock's door to the street.
        let b = st.home.unwrap().home.building;
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        let exit = *floor.exits.iter().find(|e| e.stratum == STREET).unwrap();
        put(
            &mut st,
            Place::Floor {
                building: b,
                level: 0,
            },
            exit.at,
        );
        crate::track::tick(&mut st);
        assert_eq!(map_stratum(&st), STREET);
        let area = Rect::new(0, 0, 81, 26);
        let (with, l) = draw(&st, area);
        // You show at the building's door.
        assert_eq!(l.cursor, Some(doc.tile), "the cursor starts on the target");
        let you_at = l.cell_of(exit.door).unwrap();
        assert_eq!(with[you_at].symbol(), "@");
        let mut off = st;
        let target = crate::track::target_now(&off).unwrap();
        let leg = crate::track::route::leg_from(&off, ON_STREET, exit.landing, target).unwrap();
        off.track = None;
        off.atlas.stratum = Some(STREET);
        off.atlas.cursor = l.cursor;
        let (without, _) = draw(&off, area);
        let walked: HashSet<(u16, u16)> = leg.path.iter().filter_map(|p| l.cell_of(*p)).collect();
        assert!(
            walked.iter().any(|c| with[*c] != without[*c]),
            "the walk glows"
        );
    }

    #[test]
    fn district_names_avoid_markers() {
        let mut st = outside();
        st.atlas.zoom = Zoom::Fit;
        let area = Rect::new(0, 0, 81, 26);
        let (buf, l) = draw(&st, area);
        let mut taken: HashSet<(u16, u16)> = HashSet::new();
        let (cx, cy) = l.cell_of(l.cursor.unwrap()).unwrap();
        taken.insert((cx, cy));
        for (dx, dy, ch) in [(-1, -1, "┌"), (1, -1, "┐"), (-1, 1, "└"), (1, 1, "┘")] {
            let c = ((cx as i32 + dx) as u16, (cy as i32 + dy) as u16);
            if area.contains(c.into()) {
                assert_eq!(buf[c].symbol(), ch);
                assert_eq!(buf[c].fg, CYAN);
                taken.insert(c);
            }
        }
        // Every marker shows whole, but where a corner stands on it.
        let corners = taken.clone();
        let all = listings(&st, STREET);
        for m in markers(&st, &l, &all) {
            if m.cell == (cx, cy) || !corners.contains(&m.cell) {
                assert_eq!(buf[m.cell].symbol(), m.glyph.to_string(), "{m:?}");
            }
            taken.insert(m.cell);
        }
        // Each name drawn whole covers none of them.
        let city = st.city.as_ref().unwrap();
        let mut found = 0;
        for y in area.y..area.bottom() {
            let row: Vec<char> = (area.x..area.right())
                .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                .collect();
            for d in &city.districts {
                let name: Vec<char> = d.name.to_uppercase().chars().collect();
                let Some(at) = row.windows(name.len()).position(|w| w == name.as_slice()) else {
                    continue;
                };
                found += 1;
                for k in 0..name.len() as u16 {
                    assert!(!taken.contains(&(area.x + at as u16 + k, y)), "{}", d.name);
                }
            }
        }
        assert!(found >= city.districts.len() / 2, "{found} names");
    }
}
