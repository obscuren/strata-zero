//! The Keyjack's fuses drawn: a cartridge fuse, metal caps either end of a body with
//! the filament inside, large for the blueprint window and small in the Keyjack's
//! holder. One drawing, tinted per tier: the stock fuse clear glass with silver caps,
//! the ceramic a white body, the slow-blow a coiled amber filament, the wideband cyan
//! glass round a fat wire. Pixels are half cells, folded into half blocks; text goes
//! over the folded cells.

use super::{INK, Tile, rgb, sticker};
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::{HardwareLook, Mod};
use ratatui::style::Color;

// The caps and the filament are the Keyjack's holder's silver and its traces' gold.
const SILVER: Color = rgb(0xb4bcc6);
const SILVER_HI: Color = rgb(0xdfe6ec);
const SILVER_DK: Color = rgb(0x6c7078);
const GOLD: Color = rgb(0xc9a23a);
const GOLD_DK: Color = rgb(0x8a6d14);
const GLASS: Color = rgb(0x9ad4dc);
const GLASS_HI: Color = rgb(0xd4f0f4);
const GLASS_LO: Color = rgb(0x6aa8b2);
const CERAMIC: Color = rgb(0xe6e1d6);
const CERAMIC_HI: Color = rgb(0xf6f3ec);
const CERAMIC_LO: Color = rgb(0xb5ae9f);
const AMBER: Color = rgb(0xffa83a);
const AMBER_DK: Color = rgb(0xb06a14);
const CYAN: Color = rgb(0x3fd8f0);
const CYAN_HI: Color = rgb(0xa8f2ff);
const CYAN_LO: Color = rgb(0x1890a8);
const STICKER: Color = rgb(0xe8e8ec);
const LABEL: Color = rgb(0x6b7480);

/// What inside the body carries the current.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wire {
    /// Hidden in an opaque body.
    None,
    /// A straight hair of gold.
    Hair,
    /// A fat gold wire.
    Fat,
    /// A coiled amber spring.
    Coil,
}

/// A tier's tint: its body, the body's light and shade, its wire, and the name
/// printed on it.
struct Tint {
    body: Color,
    hi: Color,
    lo: Color,
    wire: Wire,
    name: &'static str,
}

/// A fuse's tint; any look that is not a fuse's draws as the stock one.
fn tint(look: HardwareLook) -> Tint {
    let glass = |wire, name| Tint {
        body: GLASS,
        hi: GLASS_HI,
        lo: GLASS_LO,
        wire,
        name,
    };
    match look {
        HardwareLook::FuseCeramic => Tint {
            body: CERAMIC,
            hi: CERAMIC_HI,
            lo: CERAMIC_LO,
            wire: Wire::None,
            name: "CERAMIC",
        },
        HardwareLook::FuseSlowBlow => glass(Wire::Coil, "SLOW-BLOW"),
        HardwareLook::FuseWideband => Tint {
            body: CYAN,
            hi: CYAN_HI,
            lo: CYAN_LO,
            wire: Wire::Fat,
            name: "WIDEBAND",
        },
        _ => glass(Wire::Hair, "STOCK"),
    }
}

/// The fuse large, 52 by 18: side on, a cap at each end, the tier's name printed on
/// its body and its first mod on a sticker.
pub(super) fn large(p: &mut Pen, look: HardwareLook, mods: &[Mod]) {
    let t = tint(look);
    let mut c = Pixels::new(52, 18);
    // The body, with its light along the top and its shade along the bottom.
    c.rect(13, 9, 39, 27, t.body);
    c.hline(13, 39, 11, t.hi);
    c.hline(13, 39, 25, t.lo);
    c.hline(13, 39, 26, t.lo);
    match t.wire {
        Wire::None => {}
        Wire::Hair | Wire::Fat => {
            c.hline(13, 39, 18, GOLD);
            if t.wire == Wire::Fat {
                c.hline(13, 39, 19, GOLD_DK);
            }
            for x in [13, 37] {
                c.rect(x, 17, x + 2, 20, GOLD);
            }
        }
        Wire::Coil => {
            c.hline(13, 17, 18, AMBER);
            c.hline(35, 39, 18, AMBER);
            for x in 17..35 {
                let y = [15, 16, 17, 18, 19, 20, 19, 18, 17, 16][((x - 17) % 10) as usize];
                c.px(x, y, Some(AMBER));
                c.px(x, y + 1, Some(AMBER_DK));
            }
        }
    }
    // The caps, crimped where they grip the body.
    for (x0, crimp) in [(3, 11), (39, 40)] {
        c.rect(x0, 7, x0 + 10, 29, SILVER);
        c.hline(x0, x0 + 10, 8, SILVER_HI);
        c.hline(x0, x0 + 10, 27, SILVER_DK);
        c.hline(x0, x0 + 10, 28, SILVER_DK);
        c.vline(crimp, 7, 29, SILVER_DK);
    }
    c.vline(3, 8, 28, SILVER_DK);
    c.vline(48, 8, 28, SILVER_DK);
    for (x, y) in [(3, 7), (48, 7), (3, 28), (48, 28)] {
        c.px(x, y, None);
    }
    c.fold(p, 0, 0);
    // Writing: the cap's stamp, the tier on the body, the mod on a sticker.
    p.put(5, 7, "KJ-4", INK, Some(SILVER), true);
    p.put(5, 9, "250V", SILVER_DK, Some(SILVER), false);
    p.put(15, 11, t.name, INK, Some(t.body), true);
    let tag = sticker(mods);
    if !tag.is_empty() {
        p.put(24, 6, &format!(" {tag} "), INK, Some(STICKER), true);
    }
    p.put(3, 16, "▲ CAP", LABEL, None, false);
    let inside = if t.wire == Wire::None {
        "▲ BODY"
    } else {
        "▲ FILAMENT"
    };
    p.put(20, 16, inside, LABEL, None, false);
}

/// The fuse in the Keyjack's holder, 13 by 3 from the pen's origin: the body between
/// its caps. The bottom row's lower half is left to the holder under it.
pub(super) fn fitted(p: &mut Pen, look: HardwareLook) {
    let t = tint(look);
    let mut c = Pixels::new(13, 3);
    c.rect(2, 0, 11, 5, t.body);
    match t.wire {
        Wire::None => c.hline(2, 11, 4, t.lo),
        Wire::Hair => c.hline(2, 11, 2, GOLD),
        Wire::Fat => {
            c.hline(2, 11, 2, GOLD);
            c.hline(2, 11, 3, GOLD_DK);
        }
        Wire::Coil => {
            c.hline(2, 11, 2, AMBER_DK);
            for x in 2..11 {
                c.px(x, if x % 2 == 0 { 1 } else { 3 }, Some(AMBER));
            }
        }
    }
    c.rect(0, 0, 3, 5, SILVER);
    c.rect(10, 0, 13, 5, SILVER);
    c.fold(p, 0, 0);
}

/// The tile a fuse wears in the tray: its cap over its body, or its coil over the
/// glass for the slow-blow.
pub(super) fn tile(look: HardwareLook) -> Tile {
    let t = tint(look);
    let (bg, ink) = match look {
        HardwareLook::FuseCeramic => (CERAMIC, rgb(0x1a1e24)),
        HardwareLook::FuseSlowBlow => (rgb(0x4a2c08), rgb(0xffdcaa)),
        HardwareLook::FuseWideband => (rgb(0x0b4654), rgb(0xd0f8ff)),
        _ => (rgb(0x2c3e44), rgb(0xd8f0f4)),
    };
    let top = if t.wire == Wire::Coil { AMBER } else { SILVER };
    Tile {
        bg,
        ink,
        swatch: (top, t.body),
    }
}
