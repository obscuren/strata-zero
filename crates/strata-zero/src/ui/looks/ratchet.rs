//! The Ratchet SMG drawn: a wire stock, a stamped receiver, a barrel with the muzzle's
//! threads, a rail on top and a taped grip. Pixels are half cells, folded into half
//! blocks; text goes over the folded cells.

use crate::ui::cyberdeck::Pen;
use crate::ui::hardware::rgb;
use crate::ui::pixels::Pixels;
use ratatui::style::Color;

/// The drawing's width and height in cells.
pub const W: u16 = 88;
pub const H: u16 = 15;

/// Where the sockets sit, as x, y, width and height: the muzzle, then the rail.
pub const PLACES: [(i32, i32, u16, u16); 2] = [(66, 4, 22, 5), (34, 0, 24, 4)];

/// Where stickers may lie, as inclusive x0, y0, x1, y1: the receiver's side (its
/// stamping too), the grip down to the magazine's base, the stock's butt pad; less
/// the ejection port. The barrel, the wire stock, the guard and the sockets stay clear.
pub const AREA_KEEP: [(u16, u16, u16, u16); 3] = [(24, 4, 59, 8), (38, 9, 47, 14), (2, 3, 5, 10)];
pub const AREA_DROP: [(u16, u16, u16, u16); 1] = [(45, 5, 54, 6)];

// Gunmetal, stamped and worn.
const GM: Color = rgb(0x2e3138);
const GM_HI: Color = rgb(0x4a4f58);
const GM_LO: Color = rgb(0x1b1d22);
const GM_EDGE: Color = rgb(0x6b717b);
const GM_LOWER: Color = rgb(0x25282e);
const GM_SEAM: Color = rgb(0x3d424a);
const WEAR: Color = rgb(0x8d939c);
const POLY: Color = rgb(0x17181c);
const POLY_HI: Color = rgb(0x26282d);
const BARREL: Color = rgb(0x5b616a);
const BARREL_HI: Color = rgb(0x9aa1aa);
const BARREL_LO: Color = rgb(0x3f444c);
const BLACK: Color = rgb(0x0a0b0d);
const STOCK: Color = rgb(0x555a63);
const STOCK_HI: Color = rgb(0x7d838c);
const PAD: Color = rgb(0x121316);
const TAPE: Color = rgb(0xb0203c);
const LABEL_TAPE: Color = rgb(0xd9c9a0);
const INK: Color = rgb(0x1a1612);
const TAPE_HI: Color = rgb(0xd8475f);
// The muzzle's threads are gold, as the deck's connectors.
const GOLD_HI: Color = rgb(0xe8c54a);
const GOLD_DK: Color = rgb(0x8a6d14);
// The rail is bare aluminium.
const ALU: Color = rgb(0xb4bec8);
const ALU_MID: Color = rgb(0x7d8893);

/// The gun alone at the pen's origin; its sockets are left bare.
pub fn draw(p: &mut Pen) {
    let mut c = Pixels::new(W, H);
    // The wire stock, extended, to the left.
    c.hline(5, 24, 9, STOCK_HI);
    c.hline(5, 24, 16, STOCK);
    c.vline(14, 9, 17, STOCK);
    c.rect(21, 8, 24, 18, GM_LO);
    c.px(22, 10, Some(WEAR));
    c.px(22, 15, Some(WEAR));
    c.rect(2, 6, 6, 21, PAD);
    c.vline(5, 6, 21, STOCK);
    for y in (7..21).step_by(2) {
        c.px(3, y, Some(POLY_HI));
    }
    // The receiver, a stamped box.
    c.rect(24, 8, 60, 18, GM);
    c.rect(24, 16, 60, 18, GM_LOWER);
    c.hline(24, 60, 8, GM_HI);
    c.hline(25, 59, 9, GM_EDGE);
    c.hline(24, 60, 16, GM_SEAM);
    c.hline(24, 60, 17, GM_LO);
    c.vline(59, 9, 17, GM_HI);
    c.vline(24, 9, 17, GM_LO);
    // The ejection port.
    c.rect(45, 10, 55, 12, BLACK);
    c.hline(45, 55, 12, GM_HI);
    for (x, y) in [(26, 15), (57, 15), (26, 11), (42, 15)] {
        c.px(x, y, Some(WEAR));
    }
    // The charging knob, top rear.
    c.rect(27, 6, 31, 8, GM_HI);
    c.hline(27, 31, 6, WEAR);
    // The barrel and its threads, the muzzle's connector.
    c.rect(60, 11, 66, 15, BARREL);
    c.hline(60, 66, 11, BARREL_HI);
    c.hline(60, 66, 13, BARREL_LO);
    c.hline(60, 66, 14, GM_LO);
    for x in 66..70 {
        c.vline(x, 11, 15, if x % 2 == 0 { GOLD_HI } else { GOLD_DK });
    }
    // The rail on top, the rail's connector.
    for x in 34..58 {
        c.px(x, 6, (x % 2 == 0).then_some(ALU));
        c.px(x, 7, Some(ALU_MID));
    }
    // The grip with the magazine inside, the trigger and its guard in front of it.
    c.rect(38, 18, 48, 27, POLY);
    c.vline(38, 18, 27, POLY_HI);
    for y in (19..27).step_by(2) {
        c.hline(39, 47, y, POLY_HI);
    }
    c.rect(38, 21, 48, 24, TAPE);
    c.hline(38, 48, 21, TAPE_HI);
    c.rect(39, 27, 47, 28, GM_LO);
    c.rect(35, 28, 51, 30, GM);
    c.hline(35, 51, 28, GM_HI);
    c.vline(49, 18, 24, GM_HI);
    c.hline(49, 58, 23, GM_HI);
    c.vline(58, 18, 24, GM_HI);
    c.rect(52, 18, 54, 21, WEAR);
    c.px(53, 21, Some(WEAR));
    c.fold(p, 0, 0);
    // The label tape on the magazine's base plate and the stamping on the receiver.
    p.put(36, 14, " GOOD STUFF ", INK, Some(LABEL_TAPE), false);
    p.put(28, 6, "RATCHET", WEAR, Some(GM), false);
    p.put(28, 7, "9×19", GM_EDGE, Some(GM), false);
    p.put(34, 7, "SN", GM_EDGE, Some(GM), false);
    p.put(37, 7, "░░░░", GM_EDGE, Some(GM), false);
    // The filed-down selector: a bright lever over two marks.
    p.put(46, 7, "S", GM_EDGE, Some(GM), false);
    p.put(48, 7, "▬", WEAR, Some(GM), false);
    p.put(50, 7, "∞", GM_EDGE, Some(GM), false);
}
