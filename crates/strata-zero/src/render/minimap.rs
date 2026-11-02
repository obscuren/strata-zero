//! The sidebar's MAP: a close up of the deck's map round you outdoors, the plan of the
//! floor you are on indoors, and a bottom row that says where you are and where the
//! track goes. Spec `docs/superpowers/specs/2026-11-02-minimap-design.md`.

use crate::render::Rgb;
use crate::render::map::{ConstructMap, FloorMap, MapRef, StratumMap, TileMap};
use crate::render::mappix::{Pixels, PlanColours, paint, plan_px, stratum_px};
use crate::render::style::StylePack;
use crate::track::mini::MapTrack;
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, FAINT};
use citygen::{
    Floor, FloorPlan, Grid, Pos, Rect as TileRect, RoomKind, Size, StratumKind, Terrain, Tile,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;
use std::collections::{BTreeSet, HashSet};

/// Tiles a pixel in the close up.
const CLOSE: i32 = 3;
const CORNER: Color = Color::Rgb(230, 230, 240);
const YOU: Color = Color::Rgb(255, 255, 255);
/// How far the walk's pixels go toward the tracking cyan.
const WALK: f32 = 0.75;
/// The shaft strip's columns: a rule, a floor's name in four, a space, its mark, a space.
const SHAFT_W: u16 = 8;
/// Your floor's row in the strip.
const SEL: Color = Color::Rgb(20, 32, 44);
const HOME: Color = Color::Rgb(124, 255, 154);

pub struct Minimap<'a> {
    pub map: &'a MapRef<'a>,
    /// The main viewport, in tiles.
    pub view: TileRect,
    /// Your tile, when you are on this map.
    pub player: Option<Pos>,
    pub style: &'a StylePack,
    pub track: &'a MapTrack<'a>,
}

/// Where a map is drawn: its cells, the tiles a pixel covers and the tile at its
/// top-left pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct View {
    area: Rect,
    s: i32,
    origin: Pos,
}

impl View {
    /// The pixel a tile falls in, counted from the top-left one; may be off the view.
    fn pixel_of(&self, t: Pos) -> (i32, i32) {
        (
            (t.x - self.origin.x).div_euclid(self.s),
            (t.y - self.origin.y).div_euclid(self.s),
        )
    }

    /// The cell a tile falls in, when it is in view.
    fn cell_of(&self, t: Pos) -> Option<(u16, u16)> {
        let (x, y) = self.pixel_of(t);
        let y = y.div_euclid(2);
        ((0..self.area.width as i32).contains(&x) && (0..self.area.height as i32).contains(&y))
            .then(|| (self.area.x + x as u16, self.area.y + y as u16))
    }

    /// The tile at the middle of the view.
    fn centre(&self) -> Pos {
        Pos::new(
            self.origin.x + self.area.width as i32 * self.s / 2,
            self.origin.y + self.area.height as i32 * self.s,
        )
    }

    /// The cell on the view's border where the line from its middle to `t` leaves it.
    fn border_cell(&self, t: Pos) -> Option<(u16, u16)> {
        if self.area.is_empty() {
            return None;
        }
        let c = self.centre();
        let (dx, dy) = ((t.x - c.x) as f32, (t.y - c.y) as f32);
        let half_w = ((self.area.width as i32 * self.s - self.s) / 2) as f32;
        let half_h = (self.area.height as i32 * self.s - self.s) as f32;
        let k = [(dx, half_w), (dy, half_h)]
            .into_iter()
            .filter(|(d, _)| *d != 0.0)
            .map(|(d, half)| half / d.abs())
            .fold(f32::INFINITY, f32::min);
        if !k.is_finite() {
            return None;
        }
        let at = Pos::new(c.x + (dx * k) as i32, c.y + (dy * k) as i32);
        let (x, y) = self.pixel_of(at);
        let x = x.clamp(0, self.area.width as i32 - 1);
        let y = y.div_euclid(2).clamp(0, self.area.height as i32 - 1);
        Some((self.area.x + x as u16, self.area.y + y as u16))
    }
}

/// The close up's top-left tile for `cols` × `rows` cells round `centre`, on the grid of
/// cells so a cell covers the same tiles however you got there.
fn close_up_origin(centre: Pos, cols: u16, rows: u16) -> Pos {
    let x = centre.x - cols as i32 * CLOSE / 2;
    let y = centre.y - rows as i32 * CLOSE;
    Pos::new(x - x.rem_euclid(CLOSE), y - y.rem_euclid(2 * CLOSE))
}

/// The plan's tiles a pixel: 2 when the whole plan fits `cols` × `rows` cells at 2,
/// else 3.
fn plan_scale(size: Size, cols: u16, rows: u16) -> i32 {
    let fits = size.w as i32 <= 2 * cols as i32 && size.h as i32 <= 4 * rows as i32;
    if fits { 2 } else { 3 }
}

/// A plan's first tile along one axis, `span` tiles of view over a plan `len` long:
/// centred when the plan fits, else round `at` (always in view) and clamped to the plan,
/// on a grid of `grid` tiles.
fn plan_axis(len: i32, span: i32, at: i32, grid: i32) -> i32 {
    if len <= span {
        return -((span - len) / 2);
    }
    let mut o = at - span / 2;
    o -= o.rem_euclid(grid);
    while at >= o + span {
        o += grid;
    }
    let far = len - span;
    o.clamp(0, far + (grid - far.rem_euclid(grid)) % grid)
}

/// Which room each tile of a floor is in, as `Floor::room_at` would say: a room before
/// the corridor or hall it opens off, the first listed of each.
fn room_index(floor: &Floor) -> Grid<Option<RoomKind>> {
    let mut out = Grid::filled(floor.tiles.size(), None);
    let open = |r: &&citygen::Room| matches!(r.kind, RoomKind::Corridor | RoomKind::Hall);
    let (halls, rooms): (Vec<_>, Vec<_>) = floor.rooms.iter().partition(open);
    for r in halls.into_iter().rev().chain(rooms.into_iter().rev()) {
        for p in r.rect.iter() {
            out.set(p, Some(r.kind));
        }
    }
    out
}

/// The shaft strip's rows, the top floor first, `None` a gap where floors are skipped:
/// yours and the one wanted always, then the top, the bottom, the floors next to yours
/// and to the wanted one, two from yours, then the rest nearest yours, each only while
/// the strip still fits `rows`. Too few rows even for yours and the wanted one: the gap
/// rows go, the ends first, then the wanted floor.
pub fn shaft_rows(
    lowest: i32,
    top: i32,
    you: i32,
    wanted: Option<i32>,
    rows: usize,
) -> Vec<Option<i32>> {
    let laid = |chosen: &BTreeSet<i32>| {
        let mut out = Vec::new();
        let mut above: Option<i32> = None;
        for &l in chosen.iter().rev() {
            if above.map_or(l < top, |a| a - l > 1) {
                out.push(None);
            }
            out.push(Some(l));
            above = Some(l);
        }
        if above.is_some_and(|l| l > lowest) {
            out.push(None);
        }
        out
    };
    let mut order = vec![you];
    order.extend(wanted);
    order.extend([top, lowest, you + 1, you - 1]);
    if let Some(w) = wanted {
        order.extend([w + 1, w - 1]);
    }
    order.extend([you + 2, you - 2]);
    let mut rest: Vec<i32> = (lowest..=top).collect();
    rest.sort_by_key(|l| ((l - you).abs(), -l));
    order.extend(rest);
    let on = |l: &i32| (lowest..=top).contains(l);
    let mut chosen: BTreeSet<i32> = std::iter::once(you).chain(wanted).filter(on).collect();
    for l in order.into_iter().filter(on) {
        if chosen.insert(l) && laid(&chosen).len() > rows {
            chosen.remove(&l);
        }
    }
    let mut out = laid(&chosen);
    while out.len() > rows {
        if out.first() == Some(&None) {
            out.remove(0);
        } else if out.last() == Some(&None) {
            out.pop();
        } else if let Some(i) = out.iter().position(|r| r.is_none()) {
            out.remove(i);
        } else if let Some(i) = out.iter().position(|r| *r != Some(you)) {
            out.remove(i);
        } else {
            out.truncate(rows);
        }
    }
    out
}

/// The walk's pixels taken toward the tracking cyan, once each.
fn glow_walk(px: &mut Pixels, v: &View, walk: &[Pos]) {
    let lit: HashSet<(i32, i32)> = walk.iter().map(|p| v.pixel_of(*p)).collect();
    for at in lit {
        if let Some(c) = px.get_mut(at) {
            *c = c.mix(CYAN_RGB, WALK);
        }
    }
}

/// A glyph on the cell tile `t` falls in, on the colour of the pixel under it.
fn mark(buf: &mut Buffer, px: &Pixels, v: &View, t: Pos, ch: char, fg: Color, bold: bool) {
    if let Some(cell) = v.cell_of(t) {
        put(buf, px, v, cell, v.pixel_of(t), ch, fg, bold);
    }
}

/// A glyph on `cell`, on the colour of pixel `at` (the cell's top pixel when off it).
#[allow(clippy::too_many_arguments)]
fn put(
    buf: &mut Buffer,
    px: &Pixels,
    v: &View,
    cell: (u16, u16),
    at: (i32, i32),
    ch: char,
    fg: Color,
    bold: bool,
) {
    let top = ((cell.0 - v.area.x) as i32, 2 * (cell.1 - v.area.y) as i32);
    let ground = px.get(at).or_else(|| px.get(top));
    let c = &mut buf[cell];
    c.set_char(ch).set_fg(fg);
    if let Some(g) = ground {
        c.set_bg(g.to_color());
    }
    if bold {
        c.modifier.insert(Modifier::BOLD);
    } else {
        c.modifier.remove(Modifier::BOLD);
    }
}

impl Minimap<'_> {
    /// What lies where nothing is: the sidebar's own ground.
    fn ground(&self) -> Rgb {
        self.style.background
    }

    /// The tile the map centres on: yours, else the main view's middle.
    fn focus(&self) -> Pos {
        self.player.unwrap_or_else(|| {
            Pos::new(self.view.x + self.view.w / 2, self.view.y + self.view.h / 2)
        })
    }

    /// The close up of a stratum: its pixels at 3 tiles, the main view's corners, the
    /// walk, the target, you.
    fn close_up(&self, m: &StratumMap, area: Rect, buf: &mut Buffer) {
        let v = View {
            area,
            s: CLOSE,
            origin: close_up_origin(self.focus(), area.width, area.height),
        };
        let ground = self.ground();
        let mut px = Pixels::new(area.width as i32, 2 * area.height as i32, |x, y| {
            let (x0, y0) = (v.origin.x + x * CLOSE, v.origin.y + y * CLOSE);
            stratum_px(m.city, self.style, m.stratum, x0, y0, CLOSE).unwrap_or(ground)
        });
        glow_walk(&mut px, &v, self.track.walk);
        paint(buf, area, &px);
        let w = self.view;
        if w.w > 0 && w.h > 0 {
            for (t, ch) in [
                (Pos::new(w.x, w.y), '┌'),
                (Pos::new(w.right() - 1, w.y), '┐'),
                (Pos::new(w.x, w.bottom() - 1), '└'),
                (Pos::new(w.right() - 1, w.bottom() - 1), '┘'),
            ] {
                mark(buf, &px, &v, t, ch, CORNER, false);
            }
        }
        self.marks(buf, &px, &v);
    }

    /// A plan in `area`: the whole of it at 2 or 3 tiles a pixel, or round you when it
    /// is too big, and the walk on it. Returns the pixels and where they lie, for the
    /// marks.
    fn plan(
        &self,
        tiles: &Grid<Tile>,
        room: impl Fn(Pos) -> Option<RoomKind>,
        colours: &PlanColours,
        area: Rect,
        buf: &mut Buffer,
    ) -> (Pixels, View) {
        let size = tiles.size();
        let s = plan_scale(size, area.width, area.height);
        let at = self.focus();
        let v = View {
            area,
            s,
            origin: Pos::new(
                plan_axis(size.w as i32, area.width as i32 * s, at.x, s),
                plan_axis(size.h as i32, area.height as i32 * 2 * s, at.y, 2 * s),
            ),
        };
        let ground = self.ground();
        let mut px = Pixels::new(area.width as i32, 2 * area.height as i32, |x, y| {
            let (x0, y0) = (v.origin.x + x * s, v.origin.y + y * s);
            plan_px(tiles, &room, colours, x0, y0, s).unwrap_or(ground)
        });
        glow_walk(&mut px, &v, self.track.walk);
        paint(buf, area, &px);
        (px, v)
    }

    /// The floor you are on: its plan, the stairs, the lift, your home's door, the
    /// target and you; the building's floors in a strip at the right when it has more
    /// than one.
    fn floor_plan(&self, m: &FloorMap, area: Rect, buf: &mut Buffer) {
        let (plan, floor) = (m.plan, m.floor);
        let shaft = plan.lowest < plan.top && area.width > 2 * SHAFT_W;
        let plan_area = Rect {
            width: area.width - if shaft { SHAFT_W } else { 0 },
            ..area
        };
        let rooms = room_index(floor);
        let colours = PlanColours::of(self.style.theme_or_neutral(Some(plan.theme)));
        let room = |p: Pos| rooms.get(p).copied().flatten();
        let (px, v) = self.plan(&floor.tiles, room, &colours, plan_area, buf);
        // The lift wins a cell it shares with the stairs, unless the walk takes the stairs.
        let stairs = floor
            .tiles
            .get(plan.stair)
            .is_some_and(|t| t.terrain == Terrain::Stair)
            .then_some((plan.stair, '≡', DIM));
        let lift = plan.lift.map(|l| (l, '⇅', CORNER));
        let ways = if self.track.walk.last() == Some(&plan.stair) {
            [lift, stairs]
        } else {
            [stairs, lift]
        };
        for (at, ch, fg) in ways.into_iter().flatten() {
            mark(buf, &px, &v, at, ch, fg, false);
        }
        for r in floor.rooms.iter().filter(|r| r.kind == RoomKind::Home) {
            let inside = self.player.is_some_and(|p| r.rect.contains(p));
            if let (Some(door), false) = (r.door, inside) {
                mark(buf, &px, &v, door, '⌂', HOME, true);
            }
        }
        self.marks(buf, &px, &v);
        if shaft {
            let strip = Rect {
                x: area.right() - SHAFT_W,
                width: SHAFT_W,
                ..area
            };
            self.shaft(plan, floor.level, strip, buf);
        }
    }

    /// The building's floors, like a lift panel: yours on the selection ground, the one
    /// the track wants `⌖` in cyan, the shaft between them lit.
    fn shaft(&self, plan: &FloorPlan, level: i32, area: Rect, buf: &mut Buffer) {
        let wanted = self.track.wanted.filter(|w| *w != level);
        let rows = shaft_rows(plan.lowest, plan.top, level, wanted, area.height as usize);
        for y in area.y..area.bottom() {
            buf[(area.x, y)].set_char('│').set_fg(FAINT);
        }
        let at = |l: i32| rows.iter().position(|r| *r == Some(l));
        let lit = match (at(level), wanted.and_then(at)) {
            (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
            _ => None,
        };
        let top = area.y + (area.height - rows.len() as u16) / 2;
        let (name_x, mark_x) = (area.x + 1, area.x + 6);
        let shaft_lit = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
        for (i, r) in rows.iter().enumerate() {
            let y = top + i as u16;
            let between = lit.is_some_and(|(a, b)| a < i && i < b);
            let name = |l: i32| format!("{:>4}", FloorPlan::label(l));
            match *r {
                None => {
                    buf[(area.x + 4, y)].set_char('┊').set_fg(FAINT);
                    if between {
                        buf[(mark_x, y)].set_char('┃').set_style(shaft_lit);
                    }
                }
                Some(l) if l == level => {
                    for x in name_x..area.right() {
                        buf[(x, y)].set_bg(SEL);
                    }
                    let bold = Style::default()
                        .fg(YOU)
                        .bg(SEL)
                        .add_modifier(Modifier::BOLD);
                    buf.set_string(name_x, y, name(l), bold);
                    buf[(mark_x, y)].set_char('@').set_fg(YOU);
                }
                Some(l) if Some(l) == wanted => {
                    buf.set_string(name_x, y, name(l), shaft_lit);
                    buf[(mark_x, y)].set_char('⌖').set_style(shaft_lit);
                }
                Some(l) => {
                    buf.set_string(name_x, y, name(l), Style::default().fg(DIM));
                    if between {
                        buf[(mark_x, y)].set_char('┃').set_style(shaft_lit);
                    } else {
                        buf[(mark_x, y)].set_char('│').set_fg(FAINT);
                    }
                }
            }
        }
    }

    /// A construct: its plan, walls and floor, in its building's colours; the target
    /// and you.
    fn construct_plan(&self, m: &ConstructMap, area: Rect, buf: &mut Buffer) {
        let theme = m.theme(m.construct.entry);
        let colours = PlanColours::of(self.style.theme_or_neutral(theme));
        let (px, v) = self.plan(&m.construct.tiles, |_| None, &colours, area, buf);
        self.marks(buf, &px, &v);
    }

    /// The target (pinned to the border when it lies beyond it), then you.
    fn marks(&self, buf: &mut Buffer, px: &Pixels, v: &View) {
        if let Some((t, glyph, colour)) = self.track.target {
            match v.cell_of(t) {
                Some(cell) => put(buf, px, v, cell, v.pixel_of(t), glyph, colour, true),
                None => {
                    if let Some(cell) = v.border_cell(t) {
                        put(buf, px, v, cell, (-1, -1), glyph, colour, true);
                    }
                }
            }
        }
        if let Some(p) = self.player {
            // Not bold: the bold @ is the one in the main view.
            mark(buf, px, v, p, '@', YOU, false);
        }
    }

    /// Where you are, for the bottom row's left.
    fn here(&self) -> Vec<Span<'static>> {
        let dim = |t: String| Span::styled(t, Style::default().fg(DIM));
        match self.map {
            MapRef::Stratum(m) => {
                let city = m.city;
                let district = city
                    .tile(m.stratum, self.focus())
                    .and_then(|t| t.district)
                    .and_then(|d| city.district_opt(d));
                match district {
                    Some(d) => {
                        let accent = self.style.theme_or_neutral(Some(d.theme)).accent;
                        vec![Span::styled(
                            d.name.clone(),
                            Style::default().fg(accent.scale(0.85).to_color()),
                        )]
                    }
                    None if m.stratum == StratumKind::Net => vec![dim("the Net".to_string())],
                    None => vec![dim(m.stratum.label().to_string())],
                }
            }
            MapRef::Floor(m) => {
                let level = Span::styled(
                    citygen::FloorPlan::label(m.floor.level),
                    Style::default().fg(YOU).add_modifier(Modifier::BOLD),
                );
                let room = self.player.and_then(|p| m.floor.room_at(p)).map(|r| {
                    if r.number > 0 {
                        format!("{} {}", r.kind.label(), r.number)
                    } else {
                        r.kind.label().to_string()
                    }
                });
                match room {
                    Some(r) => vec![level, dim(format!(" · {r}"))],
                    None => vec![level],
                }
            }
            MapRef::Construct(_) => vec![dim("construct".to_string())],
        }
    }

    /// The bottom row: where you are on the left, the track on the right, a cell in from
    /// each side; the left is cut with `…` when the two would meet.
    fn bottom_row(&self, row: Rect, buf: &mut Buffer) {
        for x in row.x..row.right() {
            buf[(x, row.y)].reset();
            buf[(x, row.y)].set_char(' ');
        }
        if row.width < 3 {
            return;
        }
        let room = row.width as usize - 2;
        let right = &self.track.right;
        let right_w: usize = right.iter().map(|s| s.content.chars().count()).sum();
        let right_w = right_w.min(room);
        let gap = usize::from(right_w > 0);
        let left_room = room.saturating_sub(right_w + gap);
        let mut x = row.x + 1;
        let mut left = left_room;
        let left_w: usize = self.here().iter().map(|s| s.content.chars().count()).sum();
        let cut = left_w > left_room;
        for span in self.here() {
            if left == 0 {
                break;
            }
            let chars: Vec<char> = span.content.chars().collect();
            let take = chars.len().min(left);
            let mut text: String = chars[..take].iter().collect();
            left -= take;
            if cut && left == 0 {
                text.pop();
                text.push('…');
            }
            buf.set_stringn(x, row.y, &text, take, span.style);
            x += take as u16;
        }
        let mut x = row.right() - 1 - right_w as u16;
        let mut room = right_w;
        for span in right {
            let n = span.content.chars().count().min(room);
            buf.set_stringn(x, row.y, &span.content, n, span.style);
            x += n as u16;
            room -= n;
        }
    }
}

impl Widget for Minimap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let map = Rect {
            height: area.height - 1,
            ..area
        };
        let row = Rect {
            y: area.bottom() - 1,
            height: 1,
            ..area
        };
        let ground = self.ground().to_color();
        for y in map.y..map.bottom() {
            for x in map.x..map.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_char(' ').set_bg(ground);
            }
        }
        match self.map {
            MapRef::Stratum(m) => self.close_up(m, map, buf),
            MapRef::Floor(m) => self.floor_plan(m, map, buf),
            MapRef::Construct(m) => self.construct_plan(m, map, buf),
        }
        self.bottom_row(row, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use crate::ui::chrome::CYAN;
    use citygen::{City, GenParams, Generator, SizePreset};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 120, h: 80 },
            districts: 4,
            ..Default::default()
        })
        .unwrap()
    }

    /// A walkable street tile with a district, near the city's middle.
    fn street_tile(c: &City) -> Pos {
        let mid = Pos::new(c.size.w as i32 / 2, c.size.h as i32 / 2);
        (0..40)
            .flat_map(|r| (-r..=r).flat_map(move |d| [mid.offset(d, r), mid.offset(r, d)]))
            .find(|p| {
                c.tile(StratumKind::Street, *p)
                    .is_some_and(|t| t.is_walkable() && t.district.is_some())
            })
            .expect("a street tile")
    }

    fn draw(c: &City, s: &StylePack, at: Pos, track: &MapTrack, w: u16, h: u16) -> Buffer {
        let map = MapRef::stratum_of(c, StratumKind::Street);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        Minimap {
            map: &map,
            view: TileRect::new(at.x - 15, at.y - 7, 30, 15),
            player: Some(at),
            style: s,
            track,
        }
        .render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    fn find(buf: &Buffer, ch: &str) -> Option<(u16, u16)> {
        let a = buf.area;
        (a.y..a.bottom())
            .flat_map(|y| (a.x..a.right()).map(move |x| (x, y)))
            .find(|at| buf[*at].symbol() == ch)
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn minimap_fits_any_area() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        for (w, h) in [(1, 1), (2, 1), (40, 2), (40, 13), (3, 3)] {
            draw(&c, &s, at, &MapTrack::default(), w, h);
        }
        // At the city's corner the view runs off the stratum: the sidebar's ground.
        let buf = draw(&c, &s, Pos::new(0, 0), &MapTrack::default(), 40, 13);
        assert_eq!(buf[(0, 0)].bg, s.background.to_color());
    }

    #[test]
    fn you_sit_in_the_middle() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        let buf = draw(&c, &s, at, &MapTrack::default(), 40, 13);
        let (x, y) = find(&buf, "@").expect("you");
        assert!(x.abs_diff(20) <= 1 && y.abs_diff(6) <= 1, "@ at {x},{y}");
        assert_eq!(buf[(x, y)].fg, YOU);
    }

    #[test]
    fn pixels_are_the_maps() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        let buf = draw(&c, &s, at, &MapTrack::default(), 40, 13);
        let o = close_up_origin(at, 40, 12);
        let px = |x: i32, y: i32| {
            stratum_px(&c, &s, StratumKind::Street, o.x + x * 3, o.y + y * 3, 3)
                .unwrap_or(s.background)
                .to_color()
        };
        for (x, y) in [(0u16, 0u16), (5, 2), (39, 11)] {
            let cell = &buf[(x, y)];
            if cell.symbol() == "▀" {
                assert_eq!(cell.fg, px(x as i32, 2 * y as i32));
            }
            if cell.symbol() == "▀" || cell.symbol() == " " {
                assert_eq!(cell.bg, px(x as i32, 2 * y as i32 + 1));
            }
        }
    }

    #[test]
    fn the_view_shows_as_corners() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        let buf = draw(&c, &s, at, &MapTrack::default(), 40, 13);
        let v = View {
            area: Rect::new(0, 0, 40, 12),
            s: 3,
            origin: close_up_origin(at, 40, 12),
        };
        let view = TileRect::new(at.x - 15, at.y - 7, 30, 15);
        for (t, ch) in [
            (Pos::new(view.x, view.y), "┌"),
            (Pos::new(view.right() - 1, view.y), "┐"),
            (Pos::new(view.x, view.bottom() - 1), "└"),
            (Pos::new(view.right() - 1, view.bottom() - 1), "┘"),
        ] {
            let cell = v.cell_of(t).expect("in the close up");
            assert_eq!(buf[cell].symbol(), ch);
            assert_eq!(buf[cell].fg, CORNER);
        }
        assert!(find(&buf, "─").is_none(), "corners only");
    }

    #[test]
    fn the_walk_is_cyan() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        let walk = [at.offset(6, 0), at.offset(7, 0), at.offset(8, 0)];
        let track = MapTrack {
            walk: &walk,
            ..MapTrack::default()
        };
        let plain = draw(&c, &s, at, &MapTrack::default(), 40, 13);
        let lit = draw(&c, &s, at, &track, 40, 13);
        let v = View {
            area: Rect::new(0, 0, 40, 12),
            s: 3,
            origin: close_up_origin(at, 40, 12),
        };
        let (px, py) = v.pixel_of(walk[1]);
        let cell = ((px) as u16, (py / 2) as u16);
        let pick = |b: &Buffer| {
            let c = &b[cell];
            if py % 2 == 0 && c.symbol() == "▀" {
                c.fg
            } else {
                c.bg
            }
        };
        let to_rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => Rgb(r, g, b),
            _ => panic!("rgb"),
        };
        assert_eq!(
            pick(&lit),
            to_rgb(pick(&plain)).mix(CYAN_RGB, WALK).to_color()
        );
    }

    #[test]
    fn a_far_target_pins_to_the_border() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        let track = MapTrack {
            target: Some((at.offset(200, 0), '+', CYAN)),
            ..MapTrack::default()
        };
        let buf = draw(&c, &s, at, &track, 40, 13);
        let (x, y) = find(&buf, "+").expect("pinned");
        assert_eq!(x, 39, "on the east edge");
        assert!(y.abs_diff(6) <= 1);
        // In view, it sits on its own cell.
        let near = at.offset(9, 3);
        let track = MapTrack {
            target: Some((near, '+', CYAN)),
            ..MapTrack::default()
        };
        let buf = draw(&c, &s, at, &track, 40, 13);
        let v = View {
            area: Rect::new(0, 0, 40, 12),
            s: 3,
            origin: close_up_origin(at, 40, 12),
        };
        assert_eq!(find(&buf, "+"), v.cell_of(near));
    }

    #[test]
    fn your_at_is_never_bold() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        // The target under you is drawn bold first; you are drawn over it.
        let track = MapTrack {
            target: Some((at, '+', CYAN)),
            ..MapTrack::default()
        };
        let buf = draw(&c, &s, at, &track, 40, 13);
        let you = find(&buf, "@").expect("you");
        assert!(!buf[you].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn the_ground_holds_still() {
        for x in 0..12 {
            let o = close_up_origin(Pos::new(100 + x, 50 + x), 40, 12);
            assert_eq!(o.x.rem_euclid(3), 0);
            assert_eq!(o.y.rem_euclid(6), 0);
            assert!((100 + x - o.x - 60).abs() < 3, "centred within a pixel");
        }
    }

    #[test]
    fn bottom_row_names_the_district() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let at = street_tile(&c);
        let d = c.district(c.tile(StratumKind::Street, at).unwrap().district.unwrap());
        let track = MapTrack {
            right: vec![
                Span::styled("+", Style::default().fg(CYAN)),
                Span::styled(" 73 m ↗", Style::default().fg(DIM)),
            ],
            ..MapTrack::default()
        };
        let buf = draw(&c, &s, at, &track, 40, 13);
        let text = row(&buf, 12);
        assert!(text.starts_with(&format!(" {}", d.name)), "{text:?}");
        assert!(text.ends_with("+ 73 m ↗ "), "{text:?}");
        let accent = s.theme_or_neutral(Some(d.theme)).accent;
        assert_eq!(buf[(1, 12)].fg, accent.scale(0.85).to_color());
        // Too narrow for both: the left is cut, the right kept whole.
        let buf = draw(&c, &s, at, &track, 14, 6);
        let text = row(&buf, 5);
        assert!(text.ends_with("+ 73 m ↗ "), "{text:?}");
        assert!(
            text.contains('…') || d.name.chars().count() <= 3,
            "{text:?}"
        );
    }

    /// The minimap of the place you are in, as the sidebar hands it over.
    fn inside(st: &crate::app::AppState, w: u16, h: u16) -> Buffer {
        let p = st.player.as_ref().unwrap();
        let city = st.city.as_ref().unwrap();
        let map = MapRef::for_place(
            city,
            &st.interiors,
            &st.constructs,
            p.place,
            st.minute_of_day(),
        )
        .expect("a map of where you are");
        let track = crate::track::mini::map_track(st, p.place);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        Minimap {
            map: &map,
            view: TileRect::new(p.pos.x - 10, p.pos.y - 5, 20, 10),
            player: Some(p.pos),
            style: &st.style,
            track: &track,
        }
        .render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    fn text(buf: &Buffer, y: u16, xs: std::ops::Range<u16>) -> String {
        xs.map(|x| buf[(x, y)].symbol()).collect()
    }

    /// Your flat's floor, its plan, and the View a 32 × `rows` plan area draws it in.
    fn flat_view(st: &crate::app::AppState, rows: u16) -> (View, FloorPlan, i32) {
        let p = st.player.as_ref().unwrap();
        let citygen::Place::Floor { building, level } = p.place else {
            panic!("not inside");
        };
        let (plan, floor) = st.interiors.floor(building, level).unwrap();
        let size = floor.tiles.size();
        let s = plan_scale(size, 32, rows);
        let v = View {
            area: Rect::new(0, 0, 32, rows),
            s,
            origin: Pos::new(
                plan_axis(size.w as i32, 32 * s, p.pos.x, s),
                plan_axis(size.h as i32, rows as i32 * 2 * s, p.pos.y, 2 * s),
            ),
        };
        (v, plan.clone(), level)
    }

    #[test]
    fn shaft_rows_choose() {
        let gap = None;
        let f = |l: i32| Some(l);
        assert_eq!(
            shaft_rows(-1, 27, 18, Some(0), 12),
            vec![
                f(27),
                gap,
                f(21),
                f(20),
                f(19),
                f(18),
                f(17),
                f(16),
                gap,
                f(1),
                f(0),
                f(-1)
            ]
        );
        assert_eq!(
            shaft_rows(-1, 27, 18, Some(0), 5),
            vec![gap, f(18), gap, f(0), f(-1)]
        );
        assert_eq!(shaft_rows(0, 3, 2, None, 12), vec![f(3), f(2), f(1), f(0)]);
        assert_eq!(shaft_rows(-2, 0, 0, Some(-2), 12), vec![f(0), f(-1), f(-2)]);
        assert_eq!(shaft_rows(0, 40, 20, None, 3), vec![gap, f(20), gap]);
        // A building that fits shows every floor.
        let all: Vec<_> = (-1..=8).rev().map(f).collect();
        assert_eq!(shaft_rows(-1, 8, 5, Some(0), 12), all);
        assert!(shaft_rows(0, 9, 4, None, 0).is_empty());
        // Too few rows: the gaps go before your floor or the wanted one does.
        assert_eq!(shaft_rows(0, 9, 4, None, 2), vec![f(4), gap]);
        assert_eq!(shaft_rows(0, 9, 4, Some(0), 2), vec![f(4), f(0)]);
        assert_eq!(shaft_rows(0, 9, 4, Some(0), 1), vec![f(4)]);
        assert_eq!(shaft_rows(0, 9, 4, Some(7), 3), vec![f(7), gap, f(4)]);
    }

    #[test]
    fn plan_axis_keeps_you_in_view() {
        // A floor 33 tiles tall in a squeezed pane (5 rows at 3) and in one row; across.
        for (len, span, grid) in [
            (99, 30, 6),
            (99, 6, 6),
            (60, 30, 6),
            (90, 96, 3),
            (90, 60, 3),
        ] {
            for at in 0..len {
                let o = plan_axis(len, span, at, grid);
                assert!(
                    o <= at && at < o + span,
                    "{len} {span} {grid}: {at} from {o}"
                );
                if len > span {
                    assert_eq!(o.rem_euclid(grid), 0, "on the grid");
                    assert!(o >= 0 && o < len, "on the floor");
                }
            }
        }
    }

    #[test]
    fn room_index_agrees_with_room_at() {
        let (mut st, _) = crate::screens::shots::tracked_city();
        let city = st.city.clone().unwrap();
        let mut floors = 0;
        for b in city
            .buildings
            .iter()
            .filter(|b| b.kind.is_closed())
            .take(40)
        {
            let Some(floor) = st.interiors.ensure(&city, b.id, 0) else {
                continue;
            };
            let index = room_index(floor);
            for (p, _) in floor.tiles.iter() {
                let want = floor.room_at(p).map(|r| r.kind);
                assert_eq!(
                    index.get(p).copied().flatten(),
                    want,
                    "{:?} at {p:?}",
                    b.kind
                );
            }
            floors += 1;
        }
        assert!(floors > 10, "{floors} floors");
    }

    #[test]
    fn the_flat_is_drawn_whole() {
        let (st, _) = crate::screens::shots::tracked_from_the_flat();
        let buf = inside(&st, 40, 13);
        let (v, plan, _) = flat_view(&st, 12);
        assert_eq!(v.s, 3, "a megablock's floor is drawn at 3");
        let (_, floor) = st
            .interiors
            .floor(plan.building, plan.home_level.unwrap())
            .unwrap();
        let size = floor.tiles.size();
        for corner in [
            Pos::new(0, 0),
            Pos::new(size.w as i32 - 1, size.h as i32 - 1),
        ] {
            assert!(v.cell_of(corner).is_some(), "the whole floor shows");
        }
        let lift = v.cell_of(plan.lift.unwrap()).unwrap();
        assert_eq!(buf[lift].symbol(), "⇅");
        let stair = v.cell_of(plan.stair).unwrap();
        assert_eq!(buf[stair].symbol(), "≡");
        let you = v.cell_of(st.player.as_ref().unwrap().pos).unwrap();
        assert_eq!(buf[you].symbol(), "@");
        // The walk to the lift is drawn in cyan.
        let track = crate::track::mini::map_track(&st, st.player.as_ref().unwrap().place);
        let step = track.walk[track.walk.len() / 2];
        let (x, y) = v.pixel_of(step);
        let cell = &buf[(x as u16, (y / 2) as u16)];
        let c = if y % 2 == 0 && cell.symbol() == "▀" {
            cell.fg
        } else {
            cell.bg
        };
        let Color::Rgb(r, g, b) = c else {
            panic!("{c:?}")
        };
        assert!(r < 100 && g > 150 && b > 150, "the walk at {step:?}: {c:?}");
    }

    #[test]
    fn the_shaft_lights_the_way() {
        let (st, _) = crate::screens::shots::tracked_from_the_flat();
        let buf = inside(&st, 40, 13);
        let (_, plan, level) = flat_view(&st, 12);
        let rows = shaft_rows(plan.lowest, plan.top, level, Some(0), 12);
        let top = (12 - rows.len() as u16) / 2;
        for y in 0..12 {
            assert_eq!(buf[(32, y)].symbol(), "│", "the strip's rule");
        }
        let row_of = |l: i32| top + rows.iter().position(|r| *r == Some(l)).unwrap() as u16;
        let (you, lobby) = (row_of(level), row_of(0));
        assert_eq!(
            text(&buf, you, 33..37),
            format!("{:>4}", FloorPlan::label(level))
        );
        assert_eq!(buf[(33, you)].bg, SEL);
        assert!(buf[(36, you)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(38, you)].symbol(), "@");
        assert_eq!(text(&buf, lobby, 33..37), "   L");
        assert_eq!(buf[(36, lobby)].fg, CYAN);
        assert_eq!(buf[(38, lobby)].symbol(), "⌖");
        for y in you + 1..lobby {
            assert_eq!(buf[(38, y)].symbol(), "┃", "row {y}");
            assert_eq!(buf[(38, y)].fg, CYAN);
        }
        // Above you, the shaft is dark.
        assert_ne!(buf[(38, you - 1)].symbol(), "┃");
    }

    #[test]
    fn a_big_floor_follows_you() {
        let (st, _) = crate::screens::shots::tracked_from_the_flat();
        let buf = inside(&st, 40, 6);
        let (v, ..) = flat_view(&st, 5);
        assert_eq!(v.s, 3);
        let you = v
            .cell_of(st.player.as_ref().unwrap().pos)
            .expect("you are in view");
        assert!(you.0 < 32 && you.1 < 5);
        assert_eq!(buf[you].symbol(), "@");
        assert_eq!(buf[(32, 0)].symbol(), "│", "the strip still shows");
    }

    #[test]
    fn one_floor_no_shaft() {
        let (mut st, _) = crate::screens::shots::tracked_city();
        let city = st.city.clone().unwrap();
        let b = city
            .buildings
            .iter()
            .filter(|b| b.kind.is_closed())
            .find(|b| {
                st.interiors.ensure(&city, b.id, 0).is_some()
                    && st
                        .interiors
                        .plan_of(b.id)
                        .is_some_and(|p| p.lowest == p.top)
            })
            .expect("a one-floor building")
            .id;
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        let size = floor.tiles.size();
        let at = (0..size.h as i32)
            .flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| floor.tiles.get(*p).is_some_and(|t| t.is_walkable()))
            .unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Floor {
            building: b,
            level: 0,
        };
        p.pos = at;
        let buf = inside(&st, 40, 13);
        assert!((0..12).all(|y| buf[(32, y)].symbol() != "│"), "no strip");
        assert_eq!(find(&buf, "@").map(|c| c.0 < 40), Some(true));
        let bottom = text(&buf, 12, 0..40);
        assert!(bottom.starts_with(" L"), "{bottom:?}");
    }

    #[test]
    fn a_construct_draws() {
        let (mut st, _) = crate::screens::shots::tracked_city();
        let city = st.city.clone().unwrap();
        let (b, entry) = city
            .buildings
            .iter()
            .find_map(|b| st.constructs.ensure(&city, b.id).map(|k| (b.id, k.entry)))
            .expect("a construct");
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Construct(b);
        p.pos = entry;
        let buf = inside(&st, 40, 13);
        assert!(find(&buf, "@").is_some());
        assert!(text(&buf, 12, 0..40).starts_with(" construct"));
        // Walls and floor both show.
        let colours: HashSet<Color> = (0..12)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .map(|at| buf[at].bg)
            .collect();
        assert!(colours.len() >= 3, "{colours:?}");
    }

    #[test]
    fn the_bottom_row_indoors() {
        let (mut st, _) = crate::screens::shots::tracked_from_the_flat();
        let (_, plan, level) = flat_view(&st, 12);
        let name = FloorPlan::label(level);
        let buf = inside(&st, 40, 13);
        let bottom = text(&buf, 12, 0..40);
        let (_, floor) = st.interiors.floor(plan.building, level).unwrap();
        let room = floor.room_at(st.player.as_ref().unwrap().pos).unwrap();
        let want = format!(" {name} · {} {}", room.kind.label(), room.number);
        assert!(bottom.starts_with(&want), "{bottom:?}");
        assert!(bottom.ends_with("⌖ lift to L "), "{bottom:?}");
        assert!(buf[(1, 12)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(1, 12)].fg, YOU);
        // In no room, the floor's name alone.
        let size = floor.tiles.size();
        let bare = (0..size.h as i32)
            .flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| {
                floor.tiles.get(*p).is_some_and(|t| t.is_walkable()) && floor.room_at(*p).is_none()
            });
        if let Some(at) = bare {
            st.player.as_mut().unwrap().pos = at;
            let buf = inside(&st, 40, 13);
            let bottom = text(&buf, 12, 0..40);
            assert!(bottom.starts_with(&format!(" {name}  ")), "{bottom:?}");
        }
    }

    #[test]
    fn the_lift_wins_a_shared_cell_unless_you_take_the_stairs() {
        let (mut st, _) = crate::screens::shots::tracked_from_the_flat();
        // At 11 rows the flat's lift and stairs fall in one cell.
        let (v, plan, _) = flat_view(&st, 11);
        let cell = v.cell_of(plan.stair).unwrap();
        assert_eq!(v.cell_of(plan.lift.unwrap()), Some(cell));
        assert_eq!(inside(&st, 40, 12)[cell].symbol(), "⇅");
        let now = st.net.now(&st);
        st.net.trace(plan.building).add(game::ALARM_HEAT, now);
        st.net.alarm(plan.building, now);
        let source = st.track.as_ref().unwrap().source;
        crate::track::set(&mut st, source).unwrap();
        assert_eq!(inside(&st, 40, 12)[cell].symbol(), "≡");
    }

    #[test]
    fn locked_lifts_send_you_down_the_stairs() {
        let (mut st, _) = crate::screens::shots::tracked_from_the_flat();
        let (_, plan, level) = flat_view(&st, 12);
        let now = st.net.now(&st);
        st.net.trace(plan.building).add(game::ALARM_HEAT, now);
        st.net.alarm(plan.building, now);
        let source = st.track.as_ref().unwrap().source;
        crate::track::set(&mut st, source).unwrap();
        let leg = crate::track::leg_now(&st).unwrap().unwrap();
        assert!(matches!(
            leg.through,
            crate::track::route::Through::Stairs { to: 0, .. }
        ));
        let buf = inside(&st, 40, 13);
        assert!(find(&buf, "≡").is_some(), "the stairs win their cell");
        let rows = shaft_rows(plan.lowest, plan.top, level, Some(0), 12);
        let top = (12 - rows.len() as u16) / 2;
        let lobby = top + rows.iter().position(|r| *r == Some(0)).unwrap() as u16;
        assert_eq!(buf[(38, lobby)].symbol(), "⌖", "the stairs want L too");
    }
}
