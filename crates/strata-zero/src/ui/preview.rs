//! An item's drawing at half size: drawn full size offscreen, read back as half-cell
//! pixels, each two by two block taken to its commonest colour.

use super::art::Art;
use super::chrome::PANEL_BG;
use super::pixels::Pixels;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::Color;

/// A colour that counts: the panel's own and `Reset` are the bare background.
fn solid(c: Color) -> Option<Color> {
    (c != PANEL_BG && c != Color::Reset).then_some(c)
}

/// What a cell shows as its top and bottom halves. A glyph that is not a half block
/// reads as its background.
fn halves(cell: &Cell) -> (Option<Color>, Option<Color>) {
    let (fg, bg) = (solid(cell.fg), solid(cell.bg));
    match cell.symbol() {
        "▀" => (fg, bg),
        "▄" => (bg, fg),
        "█" => (fg, fg),
        _ => (bg, bg),
    }
}

/// The colour a block of pixels shrinks to: empty unless two of them are coloured,
/// otherwise the commonest, then the brighter, then the first seen.
fn commonest(block: [Option<Color>; 4]) -> Option<Color> {
    let seen: Vec<Color> = block.into_iter().flatten().collect();
    if seen.len() < 2 {
        return None;
    }
    let light = |c: Color| match c {
        Color::Rgb(r, g, b) => u32::from(r) + u32::from(g) + u32::from(b),
        _ => 0,
    };
    let mut best: Option<(usize, u32, Color)> = None;
    for &c in &seen {
        let n = seen.iter().filter(|&&o| o == c).count();
        if best.is_none_or(|(bn, bl, _)| (n, light(c)) > (bn, bl)) {
            best = Some((n, light(c), c));
        }
    }
    best.map(|(_, _, c)| c)
}

/// The drawing at half size, as pixels ready to fold into a pen.
pub(crate) fn half(art: &dyn Art) -> Pixels {
    let (ex, ey, ew, eh) = art.extent();
    let area = Rect::new(0, 0, ew, eh);
    let mut buf = Buffer::empty(area);
    for cell in &mut buf.content {
        cell.set_bg(PANEL_BG);
    }
    let origin = (
        u16::try_from(-ex).unwrap_or_default(),
        u16::try_from(-ey).unwrap_or_default(),
    );
    art.draw(origin, area, &mut buf);

    // Pixels beyond the drawing, from the width padding to even, are empty.
    let pixel = |x: u16, y: u16| -> Option<Color> {
        if x >= ew || y / 2 >= eh {
            return None;
        }
        let (top, bottom) = halves(&buf[(x, y / 2)]);
        if y.is_multiple_of(2) { top } else { bottom }
    };
    let mut out = Pixels::new(ew.div_ceil(2), eh.div_ceil(2));
    // The drawing is `2 * eh` pixels tall, so its shrunk height is `eh`; an odd
    // `eh` leaves the canvas's last pixel row empty.
    for by in 0..eh {
        for bx in 0..ew.div_ceil(2) {
            let (x, y) = (2 * bx, 2 * by);
            let block = [
                pixel(x, y),
                pixel(x + 1, y),
                pixel(x, y + 1),
                pixel(x + 1, y + 1),
            ];
            out.px(i32::from(bx), i32::from(by), commonest(block));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::cyberdeck::Pen;

    const RED: Color = Color::Rgb(0xff, 0, 0);
    const BLUE: Color = Color::Rgb(0, 0, 0xff);
    const GREEN: Color = Color::Rgb(0, 0xff, 0);
    const YELLOW: Color = Color::Rgb(0xff, 0xff, 0);
    const WHITE: Color = Color::Rgb(0xff, 0xff, 0xff);
    const DARK: Color = Color::Rgb(0x10, 0x10, 0x10);
    const BRIGHT: Color = Color::Rgb(0xf0, 0xf0, 0xf0);

    /// A drawing of loose cells, each at a place from the origin.
    struct Cells {
        size: (u16, u16),
        extent: (i32, i32, u16, u16),
        cells: Vec<(i32, i32, char, Color, Color)>,
    }

    impl Cells {
        fn new(w: u16, h: u16, cells: Vec<(i32, i32, char, Color, Color)>) -> Self {
            Cells {
                size: (w, h),
                extent: (0, 0, w, h),
                cells,
            }
        }
    }

    impl Art for Cells {
        fn size(&self) -> (u16, u16) {
            self.size
        }

        fn extent(&self) -> (i32, i32, u16, u16) {
            self.extent
        }

        fn draw(&self, (ox, oy): (u16, u16), clip: Rect, buf: &mut Buffer) {
            let mut pen = Pen {
                clip: clip.intersection(buf.area),
                buf,
                ox,
                oy,
            };
            for &(x, y, ch, fg, bg) in &self.cells {
                pen.cell(x, y, ch, fg, Some(bg));
            }
        }

        fn sockets(&self) -> Vec<crate::ui::art::SocketPlace> {
            Vec::new()
        }
    }

    fn fold(px: &Pixels, w: u16, h: u16, clip: Rect) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let mut pen = Pen {
            buf: &mut buf,
            ox: 0,
            oy: 0,
            clip,
        };
        px.fold(&mut pen, 0, 0);
        buf
    }

    #[test]
    fn half_keeps_the_commonest_colour_and_drops_thin_blocks() {
        let art = Cells::new(
            4,
            2,
            vec![
                (0, 0, '▀', RED, BLUE),
                (1, 0, ' ', RED, RED),
                (2, 0, 'X', WHITE, GREEN),
                (3, 0, '▄', YELLOW, PANEL_BG),
                (0, 1, '▄', RED, PANEL_BG),
                (2, 1, '▀', DARK, BRIGHT),
                (3, 1, '▀', DARK, BRIGHT),
            ],
        );
        let px = half(&art);
        assert_eq!(px.cells(), (2, 1));
        let buf = fold(&px, 2, 1, Rect::new(0, 0, 2, 1));
        assert_eq!((buf[(0, 0)].symbol(), buf[(0, 0)].fg), ("▀", RED));
        assert_eq!(
            (buf[(1, 0)].symbol(), buf[(1, 0)].fg, buf[(1, 0)].bg),
            ("▀", GREEN, BRIGHT)
        );
    }

    #[test]
    fn half_reads_what_reaches_past_the_size() {
        let mut art = Cells::new(
            2,
            1,
            vec![
                (-2, 0, ' ', RED, RED),
                (-1, 0, ' ', RED, RED),
                (0, 0, ' ', BLUE, BLUE),
                (1, 0, ' ', BLUE, BLUE),
            ],
        );
        art.extent = (-2, 0, 4, 1);
        let px = half(&art);
        assert_eq!(px.cells(), (2, 1));
        let buf = fold(&px, 2, 1, Rect::new(0, 0, 2, 1));
        assert_eq!((buf[(0, 0)].symbol(), buf[(0, 0)].fg), ("▀", RED));
        assert_eq!((buf[(1, 0)].symbol(), buf[(1, 0)].fg), ("▀", BLUE));
    }

    #[test]
    fn odd_sizes_pad_with_empty() {
        let art = Cells::new(3, 1, (0..3).map(|x| (x, 0, ' ', RED, RED)).collect());
        let px = half(&art);
        assert_eq!(px.cells(), (2, 1));
        let buf = fold(&px, 2, 1, Rect::new(0, 0, 2, 1));
        for x in 0..2 {
            assert_eq!((buf[(x, 0)].symbol(), buf[(x, 0)].fg), ("▀", RED));
        }
    }

    #[test]
    fn a_drawing_bigger_than_its_room_folds_clipped() {
        let cells = (0..60)
            .flat_map(|y| (0..200).map(move |x| (x, y, ' ', RED, RED)))
            .collect();
        let art = Cells::new(200, 60, cells);
        let clip = Rect::new(10, 5, 20, 6);
        let buf = fold(&half(&art), 60, 40, clip);
        for y in 0..40 {
            for x in 0..60 {
                let cell = &buf[(x, y)];
                if clip.contains((x, y).into()) {
                    assert_eq!(cell.bg, RED, "({x},{y})");
                } else {
                    assert_eq!(*cell, Cell::default(), "({x},{y})");
                }
            }
        }
    }
}
