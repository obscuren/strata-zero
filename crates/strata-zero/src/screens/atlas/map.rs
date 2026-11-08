//! The map itself: pixels, markers, the cursor, district names and the tracked walk.

use super::{Cache, map_stratum};
use crate::app::AppState;
use crate::render::mappix::{Pixels, paint, stratum_px};
use crate::track::listings::{self, Kind, Listing, source_of};
use crate::track::overlay::{from_goal, glow, mix, ripple_px};
use crate::track::route::Leg;
use crate::track::{Pulse, is_tracked, leg_now, pulses_now, target_now};
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, MAGENTA_RGB, PANEL_BG, PANEL_BG_RGB};
use citygen::{City, Place, Pos, Rect as TileRect, Size, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::collections::{BTreeMap, HashMap, HashSet};

const YOU: Color = Color::Rgb(255, 255, 255);
/// Jacked in, your Net self: the blue it wears on the Net (`netlook::YOU`).
pub(super) const NET_SELF: Color = Color::Rgb(70, 140, 255);
/// Jacked in, your body where you left it.
pub(super) const BODY: Color = Color::Rgb(160, 168, 184);
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

/// Where the map was drawn: the cells, the tiles a pixel covers (`s`) or, closer in,
/// the pixels a tile spans (`m`, else 1), the tile at the top-left cell, the cursor and
/// stratum the draw settled on, and the width of the places pane beside it (none
/// folded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapLayout {
    pub area: Rect,
    pub s: i32,
    pub m: i32,
    pub origin: Pos,
    pub cursor: Option<Pos>,
    pub stratum: Option<StratumKind>,
    pub pane: u16,
}

impl MapLayout {
    /// The cell a tile falls in, when it is in view: the middle one of those it spans.
    pub fn cell_of(&self, tile: Pos) -> Option<(u16, u16)> {
        let m = self.m.max(1);
        let (x, y) = self.pixel_of(tile);
        let (x, y) = (x + m / 2, (y + m / 2).div_euclid(2));
        ((0..self.area.width as i32).contains(&x) && (0..self.area.height as i32).contains(&y))
            .then(|| (self.area.x + x as u16, self.area.y + y as u16))
    }

    /// The tiles a cell covers: `s` × `2s`, or the one tile under it closer in.
    pub fn tiles_of(&self, (x, y): (u16, u16)) -> TileRect {
        let (s, m) = (self.s.max(1), self.m.max(1));
        // the cell is a pixel across and two down
        let (px, py) = (
            x as i32 - self.area.x as i32,
            2 * (y as i32 - self.area.y as i32),
        );
        let (x0, x1) = ((px * s).div_euclid(m), ((px + 1) * s - 1).div_euclid(m));
        let (y0, y1) = ((py * s).div_euclid(m), ((py + 2) * s - 1).div_euclid(m));
        TileRect::new(
            self.origin.x + x0,
            self.origin.y + y0,
            x1 - x0 + 1,
            y1 - y0 + 1,
        )
    }

    /// The pixel a tile falls in, counted from the top-left one (closer in, the top-left
    /// of the `m` × `m` it spans); may be out of view.
    fn pixel_of(&self, tile: Pos) -> (i32, i32) {
        let (s, m) = (self.s.max(1), self.m.max(1));
        (
            ((tile.x - self.origin.x) * m).div_euclid(s),
            ((tile.y - self.origin.y) * m).div_euclid(s),
        )
    }

    /// The tile pixel `(x, y)` lies on, the first of the `s` × `s` it covers.
    fn tile_at(&self, x: i32, y: i32) -> Pos {
        let (s, m) = (self.s.max(1), self.m.max(1));
        Pos::new(
            self.origin.x + (x * s).div_euclid(m),
            self.origin.y + (y * s).div_euclid(m),
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

/// Where your `@` shows on stratum `k`'s map: you, or jacked in, your body.
fn you_on(st: &AppState, k: StratumKind) -> Option<Pos> {
    listings::standing(st, k).map(|s| s.door)
}

/// Jacked in, where your Net self shows on the city's maps: its tile on the Net, which
/// lies over the city tile for tile, or the gate of the construct it's in.
pub fn net_self(st: &AppState) -> Option<Pos> {
    let p = st.player.as_ref()?;
    match p.place {
        Place::Stratum(StratumKind::Net) => Some(p.pos),
        Place::Construct(b) => crate::constructs::city_gate(st.city.as_ref()?, b),
        _ => None,
    }
}

/// Your `@`s on stratum `k`'s map, the one on top first: you in white; jacked in, your
/// Net self in the Net's blue over your body in grey.
fn selves(st: &AppState, k: StratumKind) -> Vec<(Pos, Color)> {
    let you = you_on(st, k);
    match net_self(st) {
        Some(net) => std::iter::once((net, NET_SELF))
            .chain(you.map(|b| (b, BODY)))
            .collect(),
        None => you.map(|p| (p, YOU)).into_iter().collect(),
    }
}

/// The walk to draw on stratum `k`'s map and the pulses on it: the leg and its pulses
/// while you are on `k`, else the walk from your building's way out and its own.
fn walk_on<'a>(
    st: &'a AppState,
    cache: &'a Cache,
    k: StratumKind,
) -> Option<(&'a Leg, &'a [Pulse])> {
    let on_k = st
        .player
        .as_ref()
        .is_some_and(|p| p.place == Place::Stratum(k));
    match leg_now(st) {
        Some(Ok(leg)) if on_k && leg.place == Place::Stratum(k) => Some((leg, pulses_now(st))),
        _ => cache.walk.as_ref().map(|w| (w, st.atlas.pulses.all())),
    }
}

/// The view's pixels, two to a cell.
fn pixels_of(st: &AppState, city: &City, k: StratumKind, l: &MapLayout) -> Pixels {
    let (w, h) = (l.area.width as i32, 2 * l.area.height as i32);
    Pixels::new(w, h, |x, y| {
        let at = l.tile_at(x, y);
        stratum_px(city, &st.style, k, at.x, at.y, l.s).unwrap_or(PANEL_BG_RGB)
    })
}

/// How much of the cyan each pixel the walk crosses takes at tick `t`, in the street's
/// floor glow: the best of the tiles it holds (closer in, every pixel a tile spans).
fn glows(l: &MapLayout, leg: &Leg, pulses: &[Pulse], t: u64) -> HashMap<(i32, i32), f32> {
    let n = leg.path.len();
    let m = l.m.max(1);
    let mut lit: HashMap<(i32, i32), f32> = HashMap::new();
    for (i, p) in leg.path.iter().enumerate() {
        let f = glow(from_goal(i, n), pulses, t);
        let (x, y) = l.pixel_of(*p);
        for (dx, dy) in (0..m).flat_map(|dy| (0..m).map(move |dx| (dx, dy))) {
            let best = lit.entry((x + dx, y + dy)).or_insert(f);
            *best = best.max(f);
        }
    }
    lit
}

/// The tracked walk on the pixels in the street's floor glow, then the ripple round the
/// target (or, when the target is not on this map, a dimmer cyan one round the way on).
fn glow_walk(
    st: &AppState,
    px: &mut Pixels,
    l: &MapLayout,
    (leg, pulses): (&Leg, &[Pulse]),
    k: StratumKind,
    quiet: Option<Rect>,
) {
    let t = st.tick;
    // Nothing moves under the card.
    let still = |(x, y): (i32, i32)| {
        quiet.is_some_and(|r| {
            let (cx, cy) = (l.area.x as i32 + x, l.area.y as i32 + y.div_euclid(2));
            (r.x as i32..r.right() as i32).contains(&cx)
                && (r.y as i32..r.bottom() as i32).contains(&cy)
        })
    };
    for (at, f) in glows(l, leg, pulses, t) {
        if !still(at)
            && let Some(c) = px.get_mut(at)
        {
            *c = c.mix(CYAN_RGB, f);
        }
    }
    let target = target_now(st).and_then(|x| listings::target_tile(st, k, x));
    let (centre, colour, strength) = match target {
        Some(p) => (p, MAGENTA_RGB, 1.0),
        None => (leg.goal, CYAN_RGB, 0.5),
    };
    // round the middle of the tile's pixels
    let (cx, cy) = l.pixel_of(centre);
    let (cx, cy) = (cx + l.m.max(1) / 2, cy + l.m.max(1) / 2);
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
    let selves = layout.stratum.map(|k| selves(st, k)).unwrap_or_default();
    for (p, colour) in selves {
        if let Some(cell) = layout.cell_of(p) {
            let m = Marker {
                cell,
                glyph: '@',
                colour,
                listing: None,
            };
            offer(0, m);
        }
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
/// view) and clamped to the stratum, its far end in view. Always on a grid of `align`
/// tiles (a multiple of `per`: the stratum's grid of cells), so a cell covers the same
/// tiles however the view got there.
fn pan(prev: i32, c: i32, len: i32, cells: i32, per: i32, align: i32, restart: bool) -> i32 {
    if len <= cells * per {
        let used = (len + per - 1) / per;
        // back from the middle to the grid, so the far end stays in view
        let back = (cells - used) / 2 * per;
        return -(back - back.rem_euclid(align));
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
    let grid = |o: i32| o - o.rem_euclid(align);
    let far = len - cells * per;
    grid(o).clamp(0, far + (align - far.rem_euclid(align)) % align)
}

/// `pan` for a cell `num` / `den` tiles long, less than a tile closer in: worked in
/// `1/den` tiles from the middle of the cursor's tile, on a grid of whole tiles, so
/// the view starts on a tile and the stratum's last tile is whole in view.
fn pan_by(prev: i32, c: i32, len: i32, cells: i32, (num, den): (i32, i32), restart: bool) -> i32 {
    let g = (1..=num.min(den))
        .rev()
        .find(|g| num % g == 0 && den % g == 0)
        .unwrap_or(1);
    let (num, den) = (num / g, den / g);
    let o = pan(
        prev * den,
        c * den + den / 2,
        len * den,
        cells,
        num,
        num * den,
        restart,
    );
    o / den
}

/// Where the map in `area` looks and where its cursor stands (spec 7): the cursor
/// starts on your Net self while you're jacked in, else on the tracked target, else on
/// you, when the map's stratum is new to it or it has none; the view pans to keep it
/// four cells inside, or centres on it when it starts or after a zoom.
pub fn layout(st: &AppState, area: Rect) -> MapLayout {
    let mut layout = MapLayout {
        area,
        s: 1,
        m: 1,
        origin: Pos::default(),
        cursor: None,
        stratum: None,
        pane: 0,
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
    let (s, m) = st
        .atlas
        .zoom
        .scale(fit_scale(size, area.width, area.height));
    let restart = st.atlas.stratum != Some(k) || st.atlas.cursor.is_none();
    let start = || {
        net_self(st)
            .or_else(|| target_now(st).and_then(|t| listings::target_tile(st, k, t)))
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
    (layout.s, layout.m) = (s, m);
    // a cell is a pixel across and two down
    layout.origin = Pos::new(
        pan_by(prev.x, cursor.x, w, cols, (s, m), centre),
        pan_by(prev.y, cursor.y, h, rows, (2 * s, m), centre),
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
    let mut px = pixels_of(st, city, k, l);
    if let Some(leg) = walk_on(st, cache, k) {
        glow_walk(st, &mut px, l, leg, k, quiet);
    }
    paint(buf, l.area, &px);
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
    use super::super::tests::{ON_STREET, STREET, jacked_in, outside, put, you};
    use super::super::{Zoom, cached};
    use super::*;
    use crate::render::Visibility;
    use crate::track::Source;
    use crate::track::listings::listings;
    use citygen::City;
    use citygen::{Terrain, Tile};
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
            Some(city) => stratum_px(city, &st.style, stratum, x0, y0, s)
                .unwrap_or(PANEL_BG_RGB)
                .to_color(),
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
        // Tiles a pixel and pixels a tile: 1, 2 and fit where fit is wider than both.
        for fit in [3, 6] {
            assert_eq!(Zoom::One.scale(fit), (1, 1));
            assert_eq!(Zoom::Two.scale(fit), (2, 1));
            assert_eq!(Zoom::Fit.scale(fit), (fit, 1));
        }
        // Fit is never further out than the whole city needs, and each zoom in is a
        // step closer: past a tile a pixel, a tile over 2 pixels, then 4.
        assert_eq!(Zoom::Fit.scale(2), (2, 1));
        assert_eq!(Zoom::Two.scale(2), (1, 1));
        assert_eq!(Zoom::One.scale(2), (1, 2));
        assert_eq!(Zoom::Fit.scale(1), (1, 1));
        assert_eq!(Zoom::Two.scale(1), (1, 2));
        assert_eq!(Zoom::One.scale(1), (1, 4));
    }

    #[test]
    fn closer_than_a_tile_a_pixel_a_tile_spans_pixels() {
        // a tile over 2 pixels: 2 cells across, a cell down; over 4: 4 across, 2 down
        for (m, across, down) in [(2, 2, 1), (4, 4, 2)] {
            let l = MapLayout {
                area: Rect::new(3, 2, 40, 20),
                s: 1,
                m,
                origin: Pos::new(10, 20),
                cursor: None,
                stratum: None,
                pane: 0,
            };
            for t in [Pos::new(10, 20), Pos::new(13, 22), Pos::new(15, 24)] {
                let cell = l.cell_of(t).expect("in view");
                assert_eq!(l.tiles_of(cell), TileRect::new(t.x, t.y, 1, 1), "{m} {t:?}");
                let next = l.cell_of(t.offset(1, 1)).unwrap();
                assert_eq!((next.0 - cell.0, next.1 - cell.1), (across, down));
                // every cell the tile spans covers it
                let (x, y) = l.pixel_of(t);
                for c in (x..x + m).flat_map(|x| (y / 2..(y + m) / 2).map(move |y| (x, y))) {
                    let c = (l.area.x + c.0 as u16, l.area.y + c.1 as u16);
                    assert_eq!(l.tiles_of(c), TileRect::new(t.x, t.y, 1, 1));
                }
            }
            assert_eq!(l.tile_at(2 * m + 1, m), Pos::new(12, 21));
        }
    }

    /// The first 2 × 2 block of street tiles of one district that `f` accepts.
    fn block(st: &AppState, f: impl Fn(&Tile) -> bool) -> Pos {
        let city = st.city.as_ref().unwrap();
        let size = city.size;
        (0..size.h as i32 - 1)
            .flat_map(|y| (0..size.w as i32 - 1).map(move |x| Pos::new(x, y)))
            .find(|p| {
                let ps = [*p, p.offset(1, 0), p.offset(0, 1), p.offset(1, 1)];
                let first = city.tile(STREET, *p).unwrap().district;
                ps.iter().all(|q| {
                    let t = city.tile(STREET, *q).unwrap();
                    f(t) && t.district == first
                })
            })
            .expect("a block of those tiles")
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

        // Whatever you have seen of it: the map draws no fog.
        let mass = block(&st, |t| t.terrain == Terrain::Mass && t.district.is_some());
        let roof = theme(&st, mass).roof;
        assert_eq!(px(&st, mass), roof.to_color());
        st.player
            .as_mut()
            .unwrap()
            .set_vis(mass.offset(1, 1), Visibility::Visible);
        assert_eq!(px(&st, mass), roof.to_color());

        let road = block(&st, |t| t.terrain == Terrain::Road && t.district.is_some());
        let lit = theme(&st, road).road.scale(1.9);
        assert_eq!(px(&st, road), lit.to_color());

        let walk = block(&st, |t| {
            t.terrain == Terrain::Sidewalk && t.district.is_some()
        });
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
    fn jacked_in_your_net_self_shows_over_your_body() {
        let mut st = outside();
        // a track set before you jacked in doesn't take the first look: you do, on the Net
        let doc = clinic(&st);
        crate::track::set(&mut st, Source::Listing(doc.id)).unwrap();
        let (body, net) = jacked_in(&mut st);
        let area = Rect::new(1, 1, 81, 26);
        let (buf, l) = draw(&st, area);
        assert_eq!(l.stratum, Some(STREET), "your body's map");
        assert_eq!(l.cursor, Some(net));
        let at = |p: Pos| {
            let c = &buf[l.cell_of(p).expect("in view")];
            (c.symbol().to_string(), c.fg)
        };
        assert_eq!(at(net), ("@".to_string(), NET_SELF));
        assert_eq!(at(body), ("@".to_string(), BODY));

        // Into a construct: your Net self at its building's gate.
        let city = st.city.as_ref().unwrap();
        let (b, gate) = city
            .buildings
            .iter()
            .find_map(|b| Some((b.id, crate::constructs::city_gate(city, b.id)?)))
            .expect("a gate");
        let p = st.player.as_mut().unwrap();
        (p.place, p.pos) = (Place::Construct(b), Pos::new(1, 1));
        assert_eq!(net_self(&st), Some(gate));

        // Back in your body: one @, white.
        let p = st.player.as_mut().unwrap();
        (p.place, p.pos) = p.body.take().unwrap();
        assert_eq!(net_self(&st), None);
        assert_eq!(selves(&st, STREET), vec![(body, YOU)]);
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
    fn the_map_walk_runs_the_same_pulses() {
        let mut st = outside();
        let doc = clinic(&st);
        crate::track::set(&mut st, Source::Listing(doc.id)).unwrap();
        st.tick += 20;
        crate::track::tick(&mut st);
        st.atlas.stratum = Some(STREET);
        st.atlas.cursor = Some(you(&st));
        let l = layout(&st, Rect::new(2, 1, 81, 26));
        let cache = cached(&st);
        let (leg, pulses) = walk_on(&st, &cache, STREET).unwrap();
        assert_eq!(crate::track::leg_now(&st), Some(Ok(leg)));
        assert_eq!(pulses, crate::track::pulses_now(&st));
        assert_eq!(pulses.len(), 1);
        // The head's pixel is as bright as the head's tile on the street, and each
        // pixel takes the best of its tiles.
        let lit = glows(&l, leg, pulses, st.tick);
        let n = leg.path.len();
        let head = pulses[0].head(st.tick);
        assert_eq!(head, n as f32 - 18.0);
        let tile = leg.path[n - 1 - head as usize];
        assert!((lit[&l.pixel_of(tile)] - 0.85).abs() < 1e-3);
        for (i, p) in leg.path.iter().enumerate() {
            assert!(lit[&l.pixel_of(*p)] >= glow(from_goal(i, n), pulses, st.tick));
        }
    }

    #[test]
    fn from_the_flat_the_map_draws_the_walk() {
        let mut st = crate::track::listings::tests::in_the_flat();
        let doc = listings(&st, STREET)
            .into_iter()
            .find(|l| l.name == "Miracorp Tattoo")
            .expect("a clinic called Miracorp Tattoo");
        crate::track::set(&mut st, Source::Listing(doc.id)).unwrap();
        crate::track::tick(&mut st);
        let at = listings::standing(&st, STREET).unwrap();
        st.atlas.stratum = Some(STREET);
        st.atlas.cursor = Some(at.door);
        let area = Rect::new(0, 0, 81, 26);
        let (with, l) = draw(&st, area);
        // You show at the building's door.
        assert_eq!(with[l.cell_of(at.door).unwrap()].symbol(), "@");
        let target = crate::track::target_now(&st).unwrap();
        let leg = crate::track::route::leg_from(&st, ON_STREET, at.landing, target).unwrap();
        assert!(leg.path.len() > 10);
        let mut off = st;
        off.track = None;
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
