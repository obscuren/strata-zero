//! The weapon pieces drawn: the Hushcan suppressor and the Redeye sight, large for
//! the blueprint window and small in their sockets on the gun. Pixels are half
//! cells, folded into half blocks; text and glyphs go over the folded cells.

use super::{ALU, ALU_DK, ALU_MID, GOLD_HI, INK, rgb, sticker};
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::Mod;
use ratatui::style::Color;

const GOLD_DK: Color = rgb(0x8a6d14);
const BLACK: Color = rgb(0x0a0b0d);
const STICKER: Color = rgb(0xe8e8ec);

const CAN: Color = rgb(0x1c1f24);
const CAN_HI: Color = rgb(0x3a3f47);
const CAN_LO: Color = rgb(0x0e1013);
const CAN_INK: Color = rgb(0x6b7480);
const CAN_SHINE: Color = rgb(0x5d6570);

const SIGHT: Color = rgb(0x15171b);
const SIGHT_HI: Color = rgb(0x3a3f47);
const LENS: Color = rgb(0xff2a3c);
const LENS_DK: Color = rgb(0x5a0c14);

const BORE: Color = rgb(0x050607);
const CAVITY: Color = rgb(0x121418);
const BAFFLE: Color = rgb(0x8d939c);
const BAFFLE_HI: Color = rgb(0xc3ccd5);
const HATCH: Color = rgb(0x3a3f47);
const SOOT: Color = rgb(0x2a2420);

const SIGHT_MID: Color = rgb(0x22252b);
const LENS_AMBER: Color = rgb(0xff8a3c);
const RUBBER: Color = rgb(0x0c0d0f);
const READOUT: Color = rgb(0xcfd6dd);

/// The Hushcan large, 72 by 14: side view, the muzzle end on the left where it screws
/// on, the lower half cut away to show the baffle stack.
pub(super) fn hushcan_large(p: &mut Pen, mods: &[Mod]) {
    let mut c = Pixels::new(72, 14);
    let (x0, x1) = (8, 70);
    // Outer wall top, bore line, outer wall bottom.
    let (top, mid, bot) = (2, 14, 26);
    // The outside, upper half: a shaded tube.
    c.rect(x0, top, x1, mid, CAN);
    c.hline(x0, x1, top, CAN_HI);
    c.hline(x0, x1, top + 1, CAN_SHINE);
    c.hline(x0, x1, top + 2, CAN_HI);
    // Rings turned into the tube.
    for x in (30..66).step_by(6) {
        c.vline(x, top + 3, mid, CAN_HI);
        c.vline(x + 1, top + 3, mid, CAN_LO);
    }
    // The cut, lower half: wall section hatched, bore channel, baffles.
    c.rect(x0, mid, x1, bot, CAVITY);
    c.hline(x0 + 6, x1, mid + 1, BORE);
    for x in x0..x1 {
        for y in [bot - 2, bot - 1] {
            c.px(x, y, Some(if (x + y) % 3 == 0 { HATCH } else { CAN_LO }));
        }
    }
    // The cut plane's edge.
    c.hline(x0, x1, mid, BAFFLE);
    // Cone baffles slanting from the wall up to the bore, opening to the muzzle end.
    for bx in (26..64).step_by(6) {
        for (i, y) in (mid + 1..=bot - 3).rev().enumerate() {
            let x = bx - i as i32 / 2;
            c.px(x, y, Some(BAFFLE));
            c.px(x + 1, y, Some(if i % 2 == 0 { BAFFLE_HI } else { BAFFLE }));
        }
        // Soot behind each baffle.
        c.px(bx + 2, bot - 4, Some(SOOT));
        c.px(bx + 3, bot - 4, Some(SOOT));
    }
    // The blast chamber behind the collar: empty, sooted.
    for x in (14..22).step_by(2) {
        c.px(x, mid + 4, Some(SOOT));
    }
    // The collar: knurled, with the gold female thread inside.
    c.rect(0, 1, x0 + 6, 27, CAN);
    for x in 0..x0 + 6 {
        for y in 1..14 {
            if (x + y) % 2 == 0 {
                c.px(x, y, Some(CAN_HI));
            }
        }
    }
    c.rect(0, 14, x0 + 6, 27, CAVITY);
    for x in 0..x0 + 6 {
        for y in 24..27 {
            c.px(x, y, Some(if (x + y) % 3 == 0 { HATCH } else { CAN_LO }));
        }
    }
    for x in 0..6 {
        c.vline(x, 15, 22, if x % 2 == 0 { GOLD_HI } else { GOLD_DK });
    }
    c.hline(0, x0 + 6, 1, CAN_SHINE);
    c.hline(0, x0 + 6, 14, BAFFLE);
    // The end cap and its exit hole.
    c.rect(x1, top + 1, x1 + 2, bot - 1, CAN_HI);
    c.px(x1 + 1, top + 1, None);
    c.px(x1 + 1, bot - 2, None);
    c.rect(x1, mid, x1 + 2, mid + 2, BORE);
    c.fold(p, 0, 0);
    // Writing.
    p.put(17, 2, "HUSH", CAN_INK, Some(CAN), true);
    p.put(22, 2, "CAN", CAN_INK, Some(CAN), false);
    p.put(17, 4, "9×19 · 1/2-28", CAN_INK, Some(CAN), false);
    let tag = sticker(mods);
    if !tag.is_empty() {
        p.put(52, 2, &format!(" {tag} "), INK, Some(STICKER), true);
    }
    p.put(26, 13, "▲ BAFFLE STACK", CAN_INK, None, false);
    p.put(0, 13, "▲ THREAD", CAN_INK, None, false);
}

/// The Hushcan on the muzzle threads, 18 by 5 from the pen's origin: a fat
/// suppressor with a collar, a ribbed body and an end cap.
pub(super) fn hushcan(p: &mut Pen) {
    let mut c = Pixels::new(18, 5);
    let (x0, x1, y0, y1) = (0, 18, 0, 10);
    c.rect(x0, y0, x1, y1, CAN);
    c.hline(x0, x1, y0, CAN_HI);
    c.hline(x0, x1, y0 + 1, CAN_SHINE);
    c.rect(x0, y1 - 3, x1, y1 - 1, CAN_LO);
    c.hline(x0, x1, y1 - 1, BLACK);
    // Knurled collar against the threads.
    for x in x0..x0 + 3 {
        for y in y0..y1 {
            if (x + y) % 2 == 0 {
                c.px(x, y, Some(CAN_HI));
            }
        }
    }
    c.vline(x0 + 3, y0, y1, CAN_LO);
    // Rings.
    for x in [9, 12, 15] {
        c.vline(x, y0 + 2, y1 - 3, CAN_HI);
    }
    // Rounded end.
    c.px(x1 - 1, y0, None);
    c.px(x1 - 1, y1 - 1, None);
    c.vline(x1 - 1, y0 + 1, y1 - 1, CAN_LO);
    c.fold(p, 0, 0);
    p.put(4, 2, "HUSH", CAN_INK, Some(CAN), false);
}

/// The Redeye large, 52 by 18: side on, clamped to a stub of rail, the hood and
/// the red front glass to the right.
pub(super) fn redeye_large(p: &mut Pen, mods: &[Mod]) {
    let mut c = Pixels::new(52, 18);
    // A stub of rail under it, for scale.
    for x in 2..50 {
        c.px(x, 32, if x % 2 == 0 { Some(ALU) } else { None });
        c.px(x, 33, Some(ALU_MID));
    }
    c.rect(2, 34, 50, 36, ALU_DK);
    // The clamp.
    c.rect(6, 26, 46, 32, ALU_DK);
    c.hline(6, 46, 26, ALU_MID);
    for x in (8..46).step_by(4) {
        c.vline(x, 30, 32, BLACK);
    }
    // Cross-bolt knob, left.
    c.rect(1, 25, 6, 31, ALU_MID);
    c.hline(1, 6, 25, ALU);
    c.vline(3, 26, 30, ALU_DK);
    // Body.
    c.rect(8, 10, 38, 26, SIGHT);
    c.hline(8, 38, 10, SIGHT_HI);
    c.hline(8, 38, 11, SIGHT_MID);
    c.hline(8, 38, 25, RUBBER);
    // Hood, taller, to the front.
    c.rect(36, 3, 48, 26, SIGHT);
    c.hline(36, 48, 3, SIGHT_HI);
    c.vline(36, 4, 10, SIGHT_HI);
    // Front glass, side on: coated, red with an amber cast.
    c.vline(48, 3, 26, LENS);
    c.vline(49, 4, 25, LENS_DK);
    c.px(48, 6, Some(LENS_AMBER));
    c.px(48, 7, Some(LENS_AMBER));
    // Windage turret on top.
    c.rect(18, 5, 27, 10, SIGHT_MID);
    c.hline(18, 27, 5, SIGHT_HI);
    for x in (19..27).step_by(2) {
        c.vline(x, 6, 10, SIGHT_HI);
    }
    // Battery cap: a disc with a coin slot.
    for y in 13..23 {
        for x in 11..21 {
            let (dx, dy) = (f64::from(x) - 15.5, f64::from(y) - 17.5);
            if dx * dx + dy * dy <= 22.0 {
                c.px(x, y, Some(SIGHT_MID));
            }
        }
    }
    c.hline(13, 19, 17, BLACK);
    c.hline(13, 19, 18, SIGHT_HI);
    c.fold(p, 0, 0);
    // Writing.
    p.cell(31, 10, '●', LENS, Some(SIGHT));
    p.put(23, 7, "REDEYE", READOUT, Some(SIGHT), true);
    p.put(23, 8, "2 MOA", CAN_INK, Some(SIGHT), false);
    p.put(23, 10, "+", READOUT, Some(SIGHT_MID), false);
    p.put(25, 10, "−", READOUT, Some(SIGHT_MID), false);
    let tag = sticker(mods);
    if !tag.is_empty() {
        p.put(37, 5, &format!(" {tag} "), INK, Some(STICKER), true);
    }
}

/// The Redeye clamped to the rail, 24 by 3 from the pen's origin: a hood to the
/// front and a red lens edge.
pub(super) fn redeye(p: &mut Pen) {
    let mut c = Pixels::new(24, 3);
    // Clamp foot over the rail teeth.
    c.rect(6, 4, 20, 6, ALU_DK);
    c.hline(6, 20, 4, ALU_MID);
    c.px(7, 5, Some(ALU));
    // Body.
    c.rect(8, 1, 18, 4, SIGHT);
    c.hline(8, 18, 1, SIGHT_HI);
    // Hood, taller, to the front.
    c.rect(18, 0, 21, 4, SIGHT);
    c.hline(18, 21, 0, SIGHT_HI);
    c.vline(21, 0, 4, LENS);
    // Turret knob.
    c.rect(11, 0, 14, 1, SIGHT_HI);
    c.fold(p, 0, 0);
    p.cell(15, 1, '●', LENS, Some(SIGHT));
}
