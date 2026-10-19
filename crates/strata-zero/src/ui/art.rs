//! What any item's drawing shares with the blueprint window: its size and place,
//! where its sockets sit, how a pointer moves between places on it, and the grey
//! the modding mode turns it.

use game::SocketKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// An item's drawing as the blueprint window's modes see it. Modding knows an item
/// only through this, so any item with sockets and a drawing can be modded.
pub trait Art {
    /// The drawing's width and height.
    fn size(&self) -> (u16, u16);

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

#[cfg(test)]
mod tests {
    use super::*;

    const PANEL: Color = Color::Rgb(4, 6, 10);

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
