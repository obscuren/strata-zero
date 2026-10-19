//! The deck hardware drawn: large for the blueprint window, small in a deck's socket,
//! and as a swatch for the tray. All shapes are half blocks in exact colours.

use super::cyberdeck::Pen;
use game::{HardwareLook, Mod};
use ratatui::style::Color;

/// The large drawing's width.
#[allow(dead_code)] // until the blueprint window opens on hardware
pub const W: u16 = 52;
/// The large drawing's height.
#[allow(dead_code)] // until the blueprint window opens on hardware
pub const H: u16 = 18;

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const STEEL_HI: Color = rgb(0xc8c2d2);
const GOLD: Color = rgb(0xc9a227);
const GOLD_HI: Color = rgb(0xe8c54a);
const ALU: Color = rgb(0xb4bec8);
const ALU_MID: Color = rgb(0x7d8893);
const ALU_DK: Color = rgb(0x2b3138);
const ALU_SHADE: Color = rgb(0x5d6772);
const FIN_END: Color = rgb(0xdfe6ec);
const COPPER: Color = rgb(0xd07a3a);
const FROST: Color = rgb(0x8fd8ff);
const INK: Color = rgb(0x1a1612);
const TRAY_INK: Color = rgb(0x1a1e24);
const STICKER_WHITE: Color = rgb(0xe8e8ec);
const FAN_FRAME: Color = rgb(0x3a424b);
const BLADE: Color = rgb(0x8a96a2);
const BLADE_HI: Color = rgb(0xc3ccd5);
const GAP: Color = rgb(0x0e1114);
const HUB: Color = rgb(0x5d6772);
const VOID_RED: Color = rgb(0xb0203c);
const PCB: Color = rgb(0x1e5a34);
const PCB_DK: Color = rgb(0x123b22);
const SILK: Color = rgb(0xd8f0de);
const TRACE_LINE: Color = rgb(0x3f8f5c);
const CHIP: Color = rgb(0x141218);
const CHIP_TEXT: Color = rgb(0xcfd6dd);
const CHIP_DIM: Color = rgb(0x6b7480);
const SILVER: Color = rgb(0x9aa4ae);
const LED: Color = rgb(0x50e678);
const TAN: Color = rgb(0xc8b48a);
const HAZARD: Color = rgb(0xffcc00);
const BLACK: Color = rgb(0x000000);
const EMPTY_BG: Color = rgb(0x2a2830);

/// Which end of the deck a socket sits at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The first mod's sticker text, empty when the piece has none.
fn sticker(mods: &[Mod]) -> String {
    mods.first().map(Mod::sticker).unwrap_or_default()
}

/// A run of one glyph along row `y`.
fn run(p: &mut Pen, y: i32, xs: std::ops::Range<i32>, ch: char, fg: Color, bg: Option<Color>) {
    for x in xs {
        p.cell(x, y, ch, fg, bg);
    }
}

/// The large drawing at the pen's origin, 52 by 18.
#[allow(dead_code)] // until the blueprint window opens on hardware
pub fn draw_large(look: HardwareLook, mods: &[Mod], p: &mut Pen) {
    match look {
        HardwareLook::ColdfinI => coldfin(p, mods, false),
        HardwareLook::ColdfinII => coldfin(p, mods, true),
        HardwareLook::Sidecar => sidecar(p, mods),
    }
}

/// A heat sink from above: base plate, mounting clamp with gold contacts, fins,
/// screws and a sticker. vII swaps the two pipes for one U pipe and a fan.
fn coldfin(p: &mut Pen, mods: &[Mod], two: bool) {
    p.fill(4, 1, 46, 16, ALU_MID);
    run(p, 0, 4..50, '▄', ALU_MID, None);
    run(p, 17, 4..50, '▀', ALU_MID, None);
    // The clamp on the mounting side.
    p.fill(0, 5, 4, 8, ALU_SHADE);
    run(p, 4, 0..4, '▄', ALU_SHADE, None);
    run(p, 13, 0..4, '▀', ALU_SHADE, None);
    for y in 6..12 {
        p.cell(0, y, '▌', GOLD, Some(ALU_DK));
    }
    p.cell(2, 6, '◦', STEEL_HI, Some(ALU_SHADE));
    p.cell(2, 11, '◦', STEEL_HI, Some(ALU_SHADE));
    // Fins: a fin on top of each row and a gap below; the ends catch the light.
    for y in 2..16 {
        run(p, y, 8..47, '▀', ALU, Some(ALU_DK));
        p.cell(47, y, '▀', FIN_END, Some(ALU_DK));
    }
    if two {
        // One U pipe, bent back at the far end.
        for y in [3, 14] {
            run(p, y, 4..8, ' ', COPPER, Some(COPPER));
            run(p, y, 8..47, '▀', ALU, Some(COPPER));
        }
        for y in 3..=14 {
            let bg = if y == 3 || y == 14 { COPPER } else { ALU_DK };
            p.cell(46, y, '▐', COPPER, Some(bg));
        }
    } else {
        // Two pipes in from the base, through the fins, bent back.
        for (top, bot) in [(4, 7), (10, 13)] {
            for y in [top, bot] {
                run(p, y, 4..8, ' ', COPPER, Some(COPPER));
                run(p, y, 8..46, '▀', ALU, Some(COPPER));
            }
            for y in top..=bot {
                p.cell(45, y, '▀', ALU, Some(COPPER));
            }
        }
    }
    for (x, y) in [(5, 1), (48, 1), (5, 16), (48, 16)] {
        p.cell(x, y, '•', ALU_DK, Some(ALU_MID));
    }
    p.put(8, 1, "▲ AIRFLOW", ALU_DK, Some(ALU_MID), false);
    let (name, name_at, tag_at) = if two {
        (" COLDFIN vII ", 8, 36)
    } else {
        (" COLDFIN vI ", 9, 31)
    };
    p.put(name_at, 16, name, INK, Some(ALU_MID), true);
    let tag = sticker(mods);
    if !tag.is_empty() {
        p.put(tag_at, 16, &format!(" {tag} "), INK, Some(FROST), true);
    }
    if two {
        p.put(41, 1, "VOID", STICKER_WHITE, Some(VOID_RED), true);
        let fan = fan_pixels();
        for (r, pair) in fan.chunks(2).enumerate() {
            for (x, (&top, &bot)) in pair[0].iter().zip(&pair[1]).enumerate() {
                p.cell(22 + x as i32, 5 + r as i32, '▀', top, Some(bot));
            }
        }
    }
}

const FAN: usize = 16;

/// The fan's pixels, two to a cell row: frame, a gap ring, five curved blades round a
/// hub, and a screw in each corner.
fn fan_pixels() -> [[Color; FAN]; FAN] {
    let c = (FAN as f32 - 1.0) / 2.0;
    let rim = FAN as f32 / 2.0 - 0.6;
    let mut grid = [[FAN_FRAME; FAN]; FAN];
    for (py, line) in grid.iter_mut().enumerate() {
        for (px, pixel) in line.iter_mut().enumerate() {
            let (dx, dy) = (px as f32 - c, py as f32 - c);
            let r = dx.hypot(dy);
            *pixel = if r > rim {
                FAN_FRAME
            } else if r > rim - 0.9 {
                GAP
            } else if r < 1.0 {
                BLADE_HI
            } else if r < 2.2 {
                HUB
            } else {
                let a = dy.atan2(dx);
                let t = (a * 5.0 / std::f32::consts::TAU + r * 0.11).rem_euclid(1.0);
                if t < 0.12 {
                    BLADE_HI
                } else if t < 0.5 {
                    BLADE
                } else {
                    GAP
                }
            };
        }
    }
    for (px, py) in [(1, 1), (FAN - 2, 1), (1, FAN - 2), (FAN - 2, FAN - 2)] {
        grid[py][px] = STEEL_HI;
    }
    grid
}

/// A daughterboard from above: gold fingers, traces, the big chip with its legs, a
/// crystal, caps, an LED, silkscreen and a QC sticker.
fn sidecar(p: &mut Pen, mods: &[Mod]) {
    p.fill(2, 1, 50, 16, PCB);
    run(p, 0, 2..52, '▄', PCB, None);
    run(p, 17, 2..52, '▀', PCB, None);
    // The edge that goes into the socket: pads with gaps.
    for y in 3..15 {
        run(p, y, 0..2, '▀', GOLD, Some(PCB_DK));
    }
    p.fill(0, 2, 2, 1, PCB);
    p.fill(0, 15, 2, 1, PCB);
    for (x, y) in [(4, 1), (49, 1), (4, 16), (49, 16)] {
        p.cell(x, y, '◦', GOLD_HI, Some(PCB));
    }
    for y in [4, 6, 8, 10, 12] {
        run(p, y, 2..14, '─', TRACE_LINE, Some(PCB));
    }
    p.cell(14, 4, '┐', TRACE_LINE, Some(PCB));
    p.cell(14, 12, '┘', TRACE_LINE, Some(PCB));
    // The chip, 16 by 7, legs all round.
    p.fill(17, 5, 16, 7, CHIP);
    for x in (18..32).step_by(2) {
        p.cell(x, 4, '▄', SILVER, Some(PCB));
        p.cell(x, 12, '▀', SILVER, Some(PCB));
    }
    for y in 5..12 {
        p.cell(16, y, '▐', SILVER, Some(PCB));
        p.cell(33, y, '▌', SILVER, Some(PCB));
    }
    p.cell(18, 5, '◦', CHIP_TEXT, Some(CHIP));
    p.put(21, 7, "SDC-7", CHIP_TEXT, Some(CHIP), true);
    p.put(20, 9, "x4 · 0617", CHIP_DIM, Some(CHIP), false);
    // The crystal can and the caps.
    p.fill(38, 4, 5, 2, SILVER);
    run(p, 4, 39..42, '▄', SILVER, Some(PCB));
    run(p, 5, 39..42, '▀', SILVER, Some(PCB));
    for (x, y, ch) in [(38, 4, '▗'), (42, 4, '▖'), (38, 5, '▝'), (42, 5, '▘')] {
        p.cell(x, y, ch, SILVER, Some(PCB));
    }
    for (x, y) in [(37, 8), (39, 8), (41, 8), (37, 10), (39, 10)] {
        p.cell(x, y, '▪', TAN, Some(PCB));
    }
    for (x, y) in [(44, 8), (44, 10), (46, 9)] {
        p.cell(x, y, '▪', BLACK, Some(PCB));
    }
    run(p, 12, 35..47, '─', TRACE_LINE, Some(PCB));
    p.cell(34, 12, '╴', TRACE_LINE, Some(PCB));
    p.cell(48, 12, '●', LED, Some(PCB));
    p.put(46, 13, "ACT", SILK, Some(PCB), false);
    p.put(17, 14, "SIDECAR rev.B", SILK, Some(PCB), true);
    let tag = sticker(mods);
    if !tag.is_empty() {
        p.put(31, 14, &format!("· {tag}"), SILK, Some(PCB), false);
    }
    p.put(44, 2, " QC ", BLACK, Some(HAZARD), true);
}

/// A glyph with its foreground and background.
type Cell = (char, Color, Color);

/// The side drawing as drawn, rows 7-11, left to right, and the side it is drawn for.
/// The Coldfins read from the outer end in; the Sidecar from the inner end out.
fn native(look: HardwareLook) -> ([[Cell; 5]; 5], Side) {
    let blank: Cell = (' ', PCB, PCB);
    match look {
        HardwareLook::ColdfinI | HardwareLook::ColdfinII => {
            let mut g = [[blank; 5]; 5];
            for (y, line) in g.iter_mut().enumerate() {
                let pipe = y == 2;
                for cell in &mut line[..4] {
                    *cell = ('▀', ALU, if pipe { COPPER } else { ALU_DK });
                }
                line[4] = ('█', if pipe { COPPER } else { ALU_MID }, ALU_MID);
            }
            if look == HardwareLook::ColdfinII {
                // Two small fans in one housing: 4 pixels wide, 10 high.
                let (f, b, h, e) = (FAN_FRAME, BLADE, BLADE_HI, GAP);
                let fan = [
                    [f, f, f, f],
                    [f, b, e, f],
                    [f, h, h, f],
                    [f, e, b, f],
                    [f, f, f, f],
                ];
                for (y, line) in g.iter_mut().enumerate() {
                    let (top, bot) = match y {
                        0 => (fan[0], fan[1]),
                        1 => (fan[2], fan[3]),
                        2 => (fan[4], fan[0]),
                        3 => (fan[1], fan[2]),
                        _ => (fan[3], fan[4]),
                    };
                    for (x, cell) in line[..4].iter_mut().enumerate() {
                        *cell = ('▀', top[x], bot[x]);
                    }
                }
            }
            (g, Side::Left)
        }
        HardwareLook::Sidecar => {
            let mut g = [[blank; 5]; 5];
            for line in &mut g[1..4] {
                line[0] = ('▌', GOLD, PCB);
            }
            for line in &mut g {
                line[1..4].fill((' ', CHIP, CHIP));
            }
            g[0][1..4].fill(('▄', SILVER, PCB));
            g[3][1..4].fill(('▀', SILVER, PCB));
            g[4][1..4].fill(blank);
            g[1][1] = ('◦', CHIP_TEXT, CHIP);
            g[0][4] = ('◦', PCB_DK, PCB);
            g[1][4] = ('┊', TRACE_LINE, PCB);
            g[2][4] = ('┊', TRACE_LINE, PCB);
            g[4][1] = ('▪', TAN, PCB);
            g[4][2] = ('╌', TRACE_LINE, PCB);
            g[4][4] = ('●', LED, PCB);
            (g, Side::Right)
        }
    }
}

/// The 5 by 5 drawing for a socket whose top-left is (x, y) in pen coordinates.
pub fn draw_side(look: HardwareLook, side: Side, p: &mut Pen, x: i32, y: i32) {
    let (mut g, drawn) = native(look);
    if side != drawn {
        for line in &mut g {
            line.reverse();
            for cell in line.iter_mut() {
                cell.0 = match cell.0 {
                    '▌' => '▐',
                    '▐' => '▌',
                    c => c,
                };
            }
        }
    }
    for (j, line) in g.iter().enumerate() {
        for (i, &(ch, fg, bg)) in line.iter().enumerate() {
            p.cell(x + i as i32, y + j as i32, ch, fg, Some(bg));
        }
    }
}

/// A piece's tray tile: its background, its ink and its swatch colours.
#[allow(dead_code)] // until the modding tray wears it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub bg: Color,
    pub ink: Color,
    pub swatch: (Color, Color),
}

/// The tile of a socket with nothing in it.
#[allow(dead_code)] // until the modding tray wears it
pub const EMPTY_TILE: Tile = Tile {
    bg: EMPTY_BG,
    ink: rgb(0xa0aab9),
    swatch: (rgb(0x4a4652), EMPTY_BG),
};

/// The tile a piece of hardware wears in the tray.
#[allow(dead_code)] // until the modding tray wears it
pub fn tile(look: HardwareLook) -> Tile {
    match look {
        HardwareLook::ColdfinI | HardwareLook::ColdfinII => Tile {
            bg: ALU,
            ink: TRAY_INK,
            swatch: (ALU, ALU_DK),
        },
        HardwareLook::Sidecar => Tile {
            bg: PCB,
            ink: SILK,
            swatch: (GOLD, PCB),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn drawn(w: u16, ox: u16, f: impl FnOnce(&mut Pen)) -> Buffer {
        let area = Rect::new(0, 0, w, 20);
        let mut buf = Buffer::empty(area);
        f(&mut Pen {
            buf: &mut buf,
            ox,
            oy: 0,
            clip: area,
        });
        buf
    }

    fn text(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn large(look: HardwareLook, mods: &[Mod]) -> Buffer {
        drawn(W, 0, |p| draw_large(look, mods, p))
    }

    #[test]
    fn coldfin_large_prints_its_number() {
        let b = large(HardwareLook::ColdfinI, &[Mod::TraceCut(10)]);
        assert_eq!(b[(0, 6)].symbol(), "▌");
        assert_eq!(b[(0, 6)].fg, GOLD);
        assert_eq!(text(&b, 31, 16, 12), " TRACE −10% ");
        assert_eq!(b[(32, 16)].bg, FROST);
        assert_eq!(text(&b, 9, 16, 12), " COLDFIN vI ");
        let b = large(HardwareLook::ColdfinII, &[Mod::TraceCut(20)]);
        assert_eq!(text(&b, 36, 16, 12), " TRACE −20% ");
        assert_eq!(text(&b, 41, 1, 4), "VOID");
        let c = &b[(29, 8)];
        assert!(c.fg == BLADE_HI || c.bg == BLADE_HI);
    }

    #[test]
    fn sidecar_large() {
        let b = large(HardwareLook::Sidecar, &[Mod::Power(1)]);
        assert_eq!(text(&b, 21, 7, 5), "SDC-7");
        assert_eq!(b[(21, 7)].bg, CHIP);
        assert_eq!(text(&b, 44, 2, 4), " QC ");
        assert_eq!(b[(44, 2)].bg, HAZARD);
        assert_eq!(text(&b, 31, 14, 10), "· POWER +1");
    }

    #[test]
    fn side_drawings_mirror() {
        let left = |look| drawn(30, 10, |p| draw_side(look, Side::Left, p, -4, 7));
        let right = |look| drawn(100, 0, |p| draw_side(look, Side::Right, p, 83, 7));
        let b = left(HardwareLook::ColdfinI);
        assert_eq!(b[(10, 9)].symbol(), "█");
        assert_eq!(b[(10, 9)].fg, COPPER);
        let b = right(HardwareLook::ColdfinI);
        assert_eq!(b[(83, 8)].symbol(), "█");
        assert_eq!(b[(83, 8)].fg, ALU_MID);
        assert_eq!(b[(84, 8)].symbol(), "▀");
        assert_eq!(b[(87, 8)].symbol(), "▀");
        let b = right(HardwareLook::Sidecar);
        assert_eq!((b[(83, 8)].symbol(), b[(83, 8)].fg), ("▌", GOLD));
        let b = left(HardwareLook::Sidecar);
        assert_eq!((b[(10, 8)].symbol(), b[(10, 8)].fg), ("▐", GOLD));
        // Cell (-3, 8) holds fan pixel rows 2 and 3: blade hi over blade hi; the
        // blade itself is the bottom pixel of (-3, 7).
        let b = left(HardwareLook::ColdfinII);
        assert_eq!(b[(7, 7)].bg, BLADE);
        assert_eq!(b[(7, 8)].fg, BLADE_HI);
    }

    #[test]
    fn tiles() {
        assert_eq!(
            tile(HardwareLook::ColdfinI).bg,
            Color::Rgb(0xb4, 0xbe, 0xc8)
        );
        assert_eq!(
            tile(HardwareLook::Sidecar).ink,
            Color::Rgb(0xd8, 0xf0, 0xde)
        );
        assert_eq!(EMPTY_TILE.bg, Color::Rgb(0x2a, 0x28, 0x30));
    }

    #[test]
    fn every_glyph_is_single_width_and_every_colour_exact() {
        use unicode_width::UnicodeWidthStr;
        for look in [
            HardwareLook::ColdfinI,
            HardwareLook::ColdfinII,
            HardwareLook::Sidecar,
        ] {
            let mut bufs = vec![large(look, &[Mod::TraceCut(10)])];
            bufs.push(drawn(30, 10, |p| draw_side(look, Side::Left, p, -4, 7)));
            for b in bufs {
                for c in &b.content {
                    assert_eq!(c.symbol().width(), 1, "{:?}", c.symbol());
                    assert!(matches!(c.fg, Color::Rgb(..) | Color::Reset));
                }
            }
        }
    }
}
