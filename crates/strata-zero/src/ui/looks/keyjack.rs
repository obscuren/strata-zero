//! The Keyjack drawn: a black slab in a magenta case on a lead that ends in an
//! alligator clip. The front has the LCD, the fault lights, the strike key and the
//! badge with its fuse light; the back, opened, has the board, the chip, the cell
//! and the fuse holder, where the fuse's socket sits. Pixels are half cells, folded
//! into half blocks; text goes over the folded cells.

use crate::ui::cyberdeck::Pen;
use crate::ui::hardware::rgb;
use crate::ui::pixels::Pixels;
use ratatui::style::Color;

/// The drawing's width and height in cells, the front's and the back's.
pub const W: u16 = 60;
pub const H: u16 = 16;

/// Where the socket sits on the back, as x, y, width and height: the fuse holder.
pub const PLACES: [(i32, i32, u16, u16); 1] = [(21, 10, 17, 3)];

const CASE: Color = rgb(0x1c1726);
const CASE_HI: Color = rgb(0x2f2742);
const CASE_LO: Color = rgb(0x0f0c16);
const EDGE: Color = rgb(0xff00c8);
const EDGE_DK: Color = rgb(0x7a0064);
const LCD: Color = rgb(0x06180e);
const BEZEL: Color = rgb(0x0b0a10);
const SCREW: Color = rgb(0x8a8a9a);
const BTN: Color = rgb(0xc8108f);
const BTN_HI: Color = rgb(0xff4fd2);
const BTN2: Color = rgb(0x3a3048);
const CABLE: Color = rgb(0x2a2a32);
const CABLE_HI: Color = rgb(0x55555f);
const CLIP: Color = rgb(0xc0c4cc);
const CLIP_DK: Color = rgb(0x6c7078);
const PCB: Color = rgb(0x0f3a24);
const PCB_HI: Color = rgb(0x18522f);
const GOLD: Color = rgb(0xc9a23a);
const SILVER: Color = rgb(0xb4bcc6);
const CHIP: Color = rgb(0x0a0b0d);
const CAP: Color = rgb(0x2b4f92);
const CELL: Color = rgb(0x30303a);
const CELL_HI: Color = rgb(0x5a5a66);
const ON: Color = rgb(0x5aff96);
const DIM: Color = rgb(0x1e7a46);
const INK: Color = rgb(0x9a90b0);
/// The fuse light with no fuse in: an unlit lamp.
const OFF: Color = rgb(0x26302b);

/// The case's outline, its lit top edge and its shadowed sides.
fn shell(c: &mut Pixels) {
    c.rect(14, 4, 54, 30, CASE);
    c.hline(15, 53, 4, CASE_HI);
    c.hline(15, 53, 5, EDGE);
    c.hline(14, 54, 29, CASE_LO);
    c.vline(53, 6, 29, CASE_LO);
    c.vline(14, 6, 29, EDGE_DK);
    for (x, y) in [(14, 4), (53, 4), (14, 29), (53, 29)] {
        c.px(x, y, None);
    }
}

/// The strain relief, the lead, and an alligator clip, jaws open to the pad.
fn lead(c: &mut Pixels) {
    c.rect(11, 11, 14, 15, CASE_LO);
    c.hline(9, 11, 12, CABLE_HI);
    c.hline(9, 11, 13, CABLE);
    c.rect(7, 9, 9, 16, CLIP_DK);
    c.hline(2, 7, 9, CLIP);
    c.hline(0, 7, 10, CLIP);
    c.hline(0, 7, 15, CLIP_DK);
    c.hline(2, 7, 16, CLIP_DK);
    for x in (0..7).step_by(2) {
        c.px(x, 11, Some(CLIP));
        c.px(x + 1, 14, Some(CLIP_DK));
    }
}

/// The front at the pen's origin: the slab with its LCD, fault lights, the strike key
/// and the lead. The badge's fuse light is lit when a fuse is in.
pub fn front(p: &mut Pen, fused: bool) {
    let mut c = Pixels::new(W, H);
    shell(&mut c);
    lead(&mut c);
    for (x, y) in [(16, 7), (51, 7), (16, 27), (51, 27)] {
        c.px(x, y, Some(SCREW));
    }
    c.rect(18, 8, 47, 20, BEZEL);
    c.rect(19, 9, 46, 19, LCD);
    // The strike key and the pull key.
    c.rect(36, 20, 46, 26, BTN);
    c.hline(36, 46, 20, BTN_HI);
    c.rect(48, 21, 52, 25, BTN2);
    // Grip ridges on the right flank.
    for y in (9..19).step_by(2) {
        c.hline(48, 52, y, CASE_HI);
    }
    c.fold(p, 0, 0);
    p.put(17, 3, "▞▚ KEYJACK", EDGE, Some(CASE), true);
    p.put(29, 3, "KJ-4", INK, Some(CASE), false);
    p.put(38, 3, "FUSE", INK, Some(CASE), false);
    p.put(43, 3, "●", if fused { ON } else { OFF }, Some(CASE), false);
    p.put(20, 5, "▕─────▒▒▒▒▒▒▒──────────▏", DIM, Some(LCD), false);
    p.put(26, 5, "▒▒▒▒▒▒▒", ON, Some(LCD), false);
    p.put(22, 5, "█", rgb(0xe6ffeb), Some(LCD), false);
    p.put(22, 7, "_ _ _ _", ON, Some(LCD), true);
    p.put(37, 7, "READY", DIM, Some(LCD), false);
    p.put(19, 11, "FAULT", INK, Some(CASE), false);
    p.put(25, 11, "● ● ●", rgb(0x4a1420), Some(CASE), false);
    p.put(38, 11, "STRIKE", rgb(0xffe6f6), Some(BTN), true);
    p.put(
        19,
        13,
        "street-built · no warranty",
        rgb(0x4a4060),
        Some(CASE),
        false,
    );
}

/// The back, opened, at the pen's origin: the board, the chip, the capacitors, the
/// cell and the fuse holder, empty. A fitted fuse is drawn over the holder by its own
/// fitted drawing; the holder's clips run under its caps.
pub fn back(p: &mut Pen) {
    let mut c = Pixels::new(W, H);
    shell(&mut c);
    lead(&mut c);
    c.rect(16, 7, 52, 28, PCB);
    // Traces.
    c.hline(18, 50, 17, GOLD);
    c.vline(31, 9, 17, GOLD);
    c.vline(41, 9, 27, GOLD);
    c.hline(18, 28, 10, PCB_HI);
    c.hline(36, 50, 24, PCB_HI);
    c.vline(19, 18, 27, GOLD);
    // The chip, pins up and down.
    c.rect(21, 9, 30, 15, CHIP);
    for x in (22..30).step_by(2) {
        c.px(x, 8, Some(SILVER));
        c.px(x, 15, Some(SILVER));
    }
    // Capacitors.
    for x in [34, 37] {
        c.rect(x, 9, x + 2, 14, CAP);
        c.hline(x, x + 2, 9, SILVER);
    }
    // The cell that runs it.
    c.rect(44, 9, 51, 16, CELL);
    c.hline(44, 51, 9, CELL_HI);
    // The fuse holder's clips.
    c.rect(22, 19, 25, 26, SILVER);
    c.rect(34, 19, 37, 26, SILVER);
    // The screws, out and lying by the case.
    for (x, y) in [(16, 7), (51, 7), (16, 27), (51, 27)] {
        c.px(x, y, Some(CHIP));
    }
    for (x, y) in [(56, 26), (58, 28), (57, 30)] {
        c.px(x, y, Some(SCREW));
    }
    c.fold(p, 0, 0);
    p.put(22, 5, "KJ4", rgb(0x8a8a9a), Some(CHIP), false);
    p.put(43, 5, "3V", rgb(0x9a9aa6), Some(CELL), false);
    p.put(23, 13, "FUSE", rgb(0x7fbf8f), Some(PCB), false);
}
