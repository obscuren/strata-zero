//! The sidebar's MAP: a close up of the deck's map round you outdoors, the plan of the
//! floor you are on indoors, and a bottom row that says where you are and where the
//! track goes. Spec `docs/superpowers/specs/2026-11-02-minimap-design.md`.

use crate::render::Rgb;
use crate::render::map::{MapRef, StratumMap};
use crate::render::mappix::{Pixels, paint, stratum_px};
use crate::render::style::StylePack;
use crate::track::mini::MapTrack;
use crate::ui::chrome::{CYAN_RGB, DIM};
use citygen::{Pos, Rect as TileRect, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;
use std::collections::HashSet;

/// Tiles a pixel in the close up.
const CLOSE: i32 = 3;
const CORNER: Color = Color::Rgb(230, 230, 240);
const YOU: Color = Color::Rgb(255, 255, 255);
/// How far the walk's pixels go toward the tracking cyan.
const WALK: f32 = 0.75;

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
        if let MapRef::Stratum(m) = self.map {
            self.close_up(m, map, buf);
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
}
