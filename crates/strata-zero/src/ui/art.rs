//! What any item's drawing shares with the blueprint window: its size and place,
//! where its sockets sit, how a pointer moves between places on it, and the grey
//! the modding mode turns it.

use crate::anim::effects::pulse;
use crate::ui::chrome::CYAN_RGB;
use crate::ui::stickers;
use game::{SocketKind, StickerArt, Turn};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// An item's drawing as the blueprint window's modes see it. Modding knows an item
/// only through this, so any item with sockets and a drawing can be modded.
pub trait Art {
    /// The drawing's width and height.
    fn size(&self) -> (u16, u16);

    /// The x, y, width and height the drawing covers, from its origin. Most drawings
    /// stay inside their size, and one that reaches past it says so here.
    fn extent(&self) -> (i32, i32, u16, u16) {
        let (w, h) = self.size();
        (0, 0, w, h)
    }

    /// Where the drawing's origin goes in `body`: centred. Both modes draw from here,
    /// so switching never moves the item.
    fn origin(&self, body: Rect) -> (u16, u16) {
        let (w, h) = self.size();
        (
            body.x + body.width.saturating_sub(w) / 2,
            body.y + body.height.saturating_sub(h) / 2,
        )
    }

    /// The item as it stands, from `origin`, nothing written outside `clip`: no mode's
    /// pointer or readout.
    fn draw(&self, origin: (u16, u16), clip: Rect, buf: &mut Buffer);

    /// The item's sockets in their order, in drawing coordinates.
    fn sockets(&self) -> Vec<SocketPlace>;

    /// Where stickers may lie on the drawing; `None` for one that takes none.
    fn sticker_area(&self) -> Option<StickerArea> {
        None
    }
}

/// The drawing cells a sticker may cover: the drawing's body, less its working parts.
#[derive(Clone, Debug, PartialEq)]
pub struct StickerArea {
    w: u16,
    h: u16,
    on: Vec<bool>,
}

impl StickerArea {
    /// A `w` by `h` drawing's area: the cells of every `keep` rect less those of every
    /// `drop` rect, each rect as inclusive x0, y0, x1, y1. Cells past the drawing are
    /// never on it.
    pub fn new(
        w: u16,
        h: u16,
        keep: &[(u16, u16, u16, u16)],
        drop: &[(u16, u16, u16, u16)],
    ) -> Self {
        let mut area = StickerArea {
            w,
            h,
            on: vec![false; usize::from(w) * usize::from(h)],
        };
        for (rects, on) in [(keep, true), (drop, false)] {
            for &(x0, y0, x1, y1) in rects {
                for y in y0..=y1.min(h.saturating_sub(1)) {
                    for x in x0..=x1.min(w.saturating_sub(1)) {
                        area.on[usize::from(y) * usize::from(w) + usize::from(x)] = on;
                    }
                }
            }
        }
        area
    }

    /// The drawing's width and height.
    pub fn size(&self) -> (u16, u16) {
        (self.w, self.h)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
            return false;
        };
        x < self.w && y < self.h && self.on[usize::from(y) * usize::from(self.w) + usize::from(x)]
    }

    /// Every cell the sticker writes, with its top-left at (x, y), lies on the area.
    pub fn fits(&self, art: &StickerArt, turn: Turn, x: i32, y: i32) -> bool {
        self.covers(&stickers::footprint(art, turn), x, y)
    }

    fn covers(&self, cells: &[(u16, u16)], x: i32, y: i32) -> bool {
        cells
            .iter()
            .all(|&(cx, cy)| self.contains(x + i32::from(cx), y + i32::from(cy)))
    }

    /// The top-left where the sticker fits whose centre lies nearest `to`, in square
    /// units (a row counts twice a column); ties go to the first in reading order.
    pub fn nearest(&self, art: &StickerArt, turn: Turn, to: (f32, f32)) -> Option<(u16, u16)> {
        let cells = stickers::footprint(art, turn);
        let (w, h) = stickers::size(art, turn);
        let mut best: Option<(f32, (u16, u16))> = None;
        for y in 0..self.h {
            for x in 0..self.w {
                if !self.covers(&cells, i32::from(x), i32::from(y)) {
                    continue;
                }
                let dx = f32::from(x) + f32::from(w) / 2.0 - to.0;
                let dy = 2.0 * (f32::from(y) + f32::from(h) / 2.0 - to.1);
                let d = dx * dx + dy * dy;
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, (x, y)));
                }
            }
        }
        best.map(|(_, at)| at)
    }
}

/// Greys every cell of `body` whose drawing cell (from `origin`) is off the area, as
/// `grey_area` does: the stickers' mode shows where they may go.
pub fn grey_outside(
    buf: &mut Buffer,
    body: Rect,
    (ox, oy): (u16, u16),
    area: &StickerArea,
    panel: Color,
) {
    let body = body.intersection(buf.area);
    for y in body.top()..body.bottom() {
        for x in body.left()..body.right() {
            if area.contains(i32::from(x) - i32::from(ox), i32::from(y) - i32::from(oy)) {
                continue;
            }
            let cell = &mut buf[(x, y)];
            let (fg, bg) = (grey(cell.fg, Color::Reset), grey(cell.bg, panel));
            cell.set_fg(fg).set_bg(bg);
            cell.modifier.remove(Modifier::BOLD);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// A socket on a drawing: the rectangle the pointer rests on, in drawing coordinates
/// (x may fall left of the drawing), its name and what it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketPlace {
    /// Its place in the item's sockets.
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
    pub name: &'static str,
    pub kind: SocketKind,
}

/// The place nearest `from` in `dir` that `allow` lets the pointer rest on, or
/// `from` when there is none. Distances run in square units (a cell is twice as
/// tall as wide). A place in the 45 degree cone wins by `primary + perp`; only when
/// that cone holds none does a wider one (`perp <= 2 * primary`) count, by
/// `primary + 2 * perp`. Ties go to the earlier place.
pub fn step<P: Copy + PartialEq>(
    parts: &[(P, Rect)],
    from: P,
    dir: Dir,
    allow: impl Fn(P) -> bool,
) -> P {
    let centre = |r: &Rect| {
        (
            f32::from(r.x) + f32::from(r.width) / 2.0,
            2.0 * (f32::from(r.y) + f32::from(r.height) / 2.0),
        )
    };
    let Some((_, here)) = parts.iter().find(|(p, _)| *p == from) else {
        return from;
    };
    let (fx, fy) = centre(here);
    let mut tight: Option<(f32, P)> = None;
    let mut wide: Option<(f32, P)> = None;
    for (p, r) in parts {
        if *p == from || !allow(*p) {
            continue;
        }
        let (cx, cy) = centre(r);
        let (dx, dy) = (cx - fx, cy - fy);
        let (primary, perp) = match dir {
            Dir::Right => (dx, dy.abs()),
            Dir::Left => (-dx, dy.abs()),
            Dir::Down => (dy, dx.abs()),
            Dir::Up => (-dy, dx.abs()),
        };
        if primary <= 0.0 {
            continue;
        }
        let (tier, score) = if perp <= primary {
            (&mut tight, primary + perp)
        } else if perp <= 2.0 * primary {
            (&mut wide, primary + 2.0 * perp)
        } else {
            continue;
        };
        if tier.is_none_or(|(best, _)| score < best) {
            *tier = Some((score, *p));
        }
    }
    tight.or(wide).map_or(from, |(_, p)| p)
}

/// A colour as the modding mode greys it: its luminance, dimmed, a touch blue, so
/// the sockets left in colour are what the eye finds. `panel` stays as it is, so a
/// background the window paints itself is left alone; so does a colour without RGB.
pub fn grey(c: Color, panel: Color) -> Color {
    let Color::Rgb(r, g, b) = c else {
        return c;
    };
    if c == panel {
        return c;
    }
    let v = (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)) * 0.38;
    let channel = |x: f64| x.round().clamp(0.0, 255.0) as u8;
    Color::Rgb(channel(v), channel(v), channel(v * 1.08))
}

/// Greys every cell of `area` outside all of `keep`: every foreground, and every
/// background but `panel`. Bold goes too.
pub fn grey_area(buf: &mut Buffer, area: Rect, keep: &[Rect], panel: Color) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if keep.iter().any(|r| r.contains((x, y).into())) {
                continue;
            }
            let cell = &mut buf[(x, y)];
            // Reset is never an RGB colour, so every foreground greys.
            let (fg, bg) = (grey(cell.fg, Color::Reset), grey(cell.bg, panel));
            cell.set_fg(fg).set_bg(bg);
            cell.modifier.remove(Modifier::BOLD);
        }
    }
}

/// Four cyan heavy corners one cell outside `r`, pulsing like the selected rows on
/// the other tabs. Corners that fall outside `clip` are skipped; the cells keep
/// whatever they sat on.
pub(crate) fn brackets(buf: &mut Buffer, r: Rect, tick: u64, clip: Rect) {
    let fg = CYAN_RGB.scale(0.7 + 0.3 * pulse(tick, 30)).to_color();
    let (x1, y1) = (r.x.checked_sub(1), r.y.checked_sub(1));
    let (x2, y2) = (r.x + r.width, r.y + r.height);
    for (x, y, glyph) in [
        (x1, y1, '┏'),
        (Some(x2), y1, '┓'),
        (x1, Some(y2), '┗'),
        (Some(x2), Some(y2), '┛'),
    ] {
        if let (Some(x), Some(y)) = (x, y)
            && clip.contains((x, y).into())
        {
            buf[(x, y)].set_char(glyph).set_fg(fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::hardware::HardwareArt;
    use game::HardwareLook;

    #[test]
    fn extent_defaults_to_the_size() {
        let art = HardwareArt {
            look: HardwareLook::Redeye,
            mods: Vec::new(),
        };
        assert_eq!(art.extent(), (0, 0, 52, 18));
    }

    #[test]
    fn brackets_pulse_and_stay_on_the_buffer() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        let clip = buf.area;
        brackets(&mut buf, Rect::new(5, 3, 4, 2), 0, clip);
        assert_eq!(buf[(4, 2)].symbol(), "┏");
        assert_eq!(buf[(9, 2)].symbol(), "┓");
        assert_eq!(buf[(4, 5)].symbol(), "┗");
        assert_eq!(buf[(9, 5)].symbol(), "┛");
        let a = buf[(4, 2)].fg;
        brackets(&mut buf, Rect::new(5, 3, 4, 2), 8, clip);
        assert_ne!(buf[(4, 2)].fg, a);
        // At the buffer's edge the corners that fall outside are skipped.
        brackets(&mut buf, Rect::new(0, 0, 4, 2), 0, clip);
        assert_eq!(buf[(4, 2)].symbol(), "┛");
    }

    const PANEL: Color = Color::Rgb(4, 6, 10);

    #[test]
    fn a_sticker_fits_only_wholly_on_the_area() {
        let area = StickerArea::new(10, 4, &[(0, 0, 9, 3)], &[(4, 1, 5, 2)]);
        let two = crate::ui::stickers::tests::lettered("ab");
        assert!(area.contains(0, 0) && area.contains(9, 3));
        assert!(!area.contains(4, 1) && !area.contains(10, 0) && !area.contains(-1, 0));
        assert!(area.fits(&two, Turn::Level, 0, 0));
        assert!(!area.fits(&two, Turn::Level, 3, 1), "over the hole");
        assert!(!area.fits(&two, Turn::Level, 9, 0), "off the edge");
        assert!(!area.fits(&two, Turn::Level, -1, 0));
        // Stood up it is three wide and two tall, which the hole's rows refuse.
        assert!(area.fits(&two, Turn::Quarter, 0, 0));
        assert!(!area.fits(&two, Turn::Quarter, 3, 0));
        // Nearest by its centre, a row counting twice a column.
        assert_eq!(area.nearest(&two, Turn::Level, (4.5, 1.5)), Some((2, 1)));
        let wide = crate::ui::stickers::tests::lettered("abcdefghijk");
        assert_eq!(area.nearest(&wide, Turn::Level, (4.5, 1.5)), None);
    }

    #[test]
    fn grey_outside_spares_the_area() {
        let red = Color::Rgb(255, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
        for cell in &mut buf.content {
            cell.set_char('x').set_fg(red).set_bg(red);
            cell.modifier = Modifier::BOLD;
        }
        buf[(5, 2)].set_bg(PANEL);
        // A 2×1 area at drawing (1, 1), the drawing's origin at (2, 0).
        let area = StickerArea::new(4, 3, &[(1, 1, 2, 1)], &[]);
        grey_outside(&mut buf, Rect::new(0, 0, 6, 3), (2, 0), &area, PANEL);
        for (x, y) in [(3, 1), (4, 1)] {
            let cell = &buf[(x, y)];
            assert_eq!((cell.fg, cell.bg), (red, red), "({x},{y})");
            assert!(cell.modifier.contains(Modifier::BOLD));
        }
        for (x, y) in [(0, 0), (2, 1), (5, 1), (3, 2)] {
            let cell = &buf[(x, y)];
            assert_eq!(
                (cell.fg, cell.bg),
                (grey(red, PANEL), grey(red, PANEL)),
                "({x},{y})"
            );
            assert!(!cell.modifier.contains(Modifier::BOLD));
        }
        assert_eq!(buf[(5, 2)].bg, PANEL);
    }

    #[test]
    fn grey_reads_luminance_and_spares_the_panel() {
        // v = 0.299r + 0.587g + 0.114b, times 0.38, as (v, v, v * 1.08), rounded.
        let label = Color::Rgb(0x8b, 0x85, 0x96);
        assert_eq!(grey(label, PANEL), Color::Rgb(52, 52, 56));
        assert_eq!(
            grey(Color::Rgb(255, 255, 255), PANEL),
            Color::Rgb(97, 97, 105)
        );
        assert_eq!(grey(Color::Rgb(0, 0, 0), PANEL), Color::Rgb(0, 0, 0));
        assert_eq!(grey(PANEL, PANEL), PANEL);
        assert_eq!(grey(Color::Reset, PANEL), Color::Reset);
    }

    #[test]
    fn grey_area_keeps_what_it_is_told_and_drops_bold() {
        let (red, green) = (Color::Rgb(255, 0, 0), Color::Rgb(0, 255, 0));
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 2));
        for x in 0..5 {
            for y in 0..2 {
                let cell = &mut buf[(x, y)];
                cell.set_char('x').set_fg(red).set_bg(green);
                cell.modifier = Modifier::BOLD;
            }
        }
        buf[(3, 0)].set_bg(PANEL);
        buf[(4, 0)].set_fg(PANEL);
        grey_area(
            &mut buf,
            Rect::new(0, 0, 5, 1),
            &[Rect::new(1, 0, 1, 2)],
            PANEL,
        );
        let greyed = &buf[(0, 0)];
        assert_eq!(greyed.symbol(), "x");
        assert_eq!(greyed.fg, Color::Rgb(29, 29, 31));
        assert_eq!(greyed.bg, Color::Rgb(57, 57, 61));
        assert!(!greyed.modifier.contains(Modifier::BOLD));
        // Kept, and outside the area: untouched.
        for (x, y) in [(1, 0), (0, 1), (2, 1)] {
            let cell = &buf[(x, y)];
            assert_eq!((cell.fg, cell.bg), (red, green), "({x},{y})");
            assert!(cell.modifier.contains(Modifier::BOLD), "({x},{y})");
        }
        // The panel's background stays; a foreground always greys.
        assert_eq!(buf[(3, 0)].bg, PANEL);
        assert_eq!(buf[(4, 0)].fg, Color::Rgb(2, 2, 2));
    }
}
