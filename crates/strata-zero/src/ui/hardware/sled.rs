//! The rig's sleds drawn: a blade's dark faceplate between two rack ears, a pull
//! handle at the left, vents at the right and a row of lamps along the bottom. One
//! drawing, tinted per bay kind (CORE cyan, RELAY green, LINE amber, FIREWALL red),
//! with the tier in the lamps: one, two or three lit pairs. The rack on the desk
//! draws its sleds small in the same manner, from the same colours.

use super::{INK, Tile, rgb, sticker};
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::{HardwareLook, Mod, SocketKind};
use ratatui::style::Color;

// The rack's darks, from the plate's shadow up to its lit edge, and its steel.
pub(crate) const K0: Color = rgb(0x0b0a0d);
pub(crate) const K1: Color = rgb(0x141318);
pub(crate) const K2: Color = rgb(0x1f1d24);
pub(crate) const K3: Color = rgb(0x2b2832);
pub(crate) const ST1: Color = rgb(0x3a3d45);
pub(crate) const ST2: Color = rgb(0x555a66);
pub(crate) const ST3: Color = rgb(0x7a808c);
const STICKER: Color = rgb(0xe8e8ec);
const LABEL: Color = rgb(0x6b7480);

/// A bay kind's colour: CORE cyan, RELAY green, LINE amber, FIREWALL red. Any other
/// kind reads as a core.
pub(crate) fn kind_colour(k: SocketKind) -> Color {
    match k {
        SocketKind::Relay => rgb(0x50e678),
        SocketKind::Line => rgb(0xffb000),
        SocketKind::Firewall => rgb(0xff3a5a),
        _ => rgb(0x00e5ff),
    }
}

/// A sled's tier, 1 to 3, from the size of what it gives: seats 1/2/3, lines 3/5/8,
/// holds 3/5/7 days, firewalls 1/2/3. A speed alone says nothing; no mod is tier 1.
pub(crate) fn tier(mods: &[Mod]) -> u8 {
    let of = |m: &Mod| match *m {
        Mod::Seats(n) | Mod::Firewall(n) => n,
        Mod::Lines(n) => match n {
            0..=3 => 1,
            4..=5 => 2,
            _ => 3,
        },
        Mod::Hold(n) => match n {
            0..=3 => 1,
            4..=5 => 2,
            _ => 3,
        },
        _ => 1,
    };
    mods.iter().map(of).max().unwrap_or(1).clamp(1, 3)
}

/// The bay kind a sled's look is drawn for.
pub(crate) fn kind_of(look: HardwareLook) -> SocketKind {
    match look {
        HardwareLook::SledRelay => SocketKind::Relay,
        HardwareLook::SledLine => SocketKind::Line,
        HardwareLook::SledFirewall => SocketKind::Firewall,
        _ => SocketKind::Core,
    }
}

/// `a` taken `t` of the way to `b`.
pub(crate) fn mix(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color::Rgb(m(ar, br), m(ag, bg), m(ab, bb))
}

/// A lamp that is off: its colour sunk most of the way into the plate.
pub(crate) fn unlit(n: Color) -> Color {
    mix(n, K2, 0.72)
}

/// The sled large, 52 by 18: its faceplate front on between the rack ears, the kind
/// on the plate and what it gives on a label.
pub(super) fn large(p: &mut Pen, look: HardwareLook, mods: &[Mod]) {
    let kind = kind_of(look);
    let n = kind_colour(kind);
    let lit = tier(mods);
    let mut c = Pixels::new(52, 18);
    // The ears either side, with their screw holes.
    for x0 in [0, 46] {
        c.rect(x0, 4, x0 + 6, 32, ST1);
        c.hline(x0, x0 + 6, 4, ST2);
        c.hline(x0, x0 + 6, 31, K1);
        for y in [7, 27] {
            c.rect(x0 + 2, y, x0 + 4, y + 2, K0);
        }
    }
    // The plate, lit along its top and shaded along its foot.
    c.rect(5, 4, 47, 32, K2);
    c.hline(5, 47, 4, K3);
    c.vline(5, 4, 32, K3);
    c.hline(5, 47, 31, K0);
    c.vline(46, 4, 32, K0);
    // The pull handle: a steel bar on two standoffs, a grip hole through it.
    c.rect(8, 7, 12, 29, ST2);
    c.vline(8, 7, 29, ST3);
    c.hline(8, 12, 7, ST3);
    c.rect(9, 11, 11, 25, K0);
    c.vline(12, 8, 29, ST1);
    // Vents.
    for x in (35..45).step_by(2) {
        c.vline(x, 8, 24, K0);
    }
    // The lamps, three pairs; the tier lights that many.
    for pair in 0..3u8 {
        for k in 0..2 {
            let x = 15 + i32::from(pair) * 8 + k * 3;
            c.rect(x, 26, x + 2, 28, if pair < lit { n } else { unlit(n) });
        }
    }
    c.fold(p, 0, 0);
    p.put(15, 4, &kind.label().to_uppercase(), n, Some(K2), true);
    p.put(15, 6, "SLED", mix(n, K2, 0.45), Some(K2), false);
    let tag = sticker(mods);
    if !tag.is_empty() {
        p.put(15, 9, &format!(" {tag} "), INK, Some(STICKER), true);
    }
    p.put(4, 16, "▲ HANDLE", LABEL, None, false);
    p.put(15, 16, "▲ LAMPS", LABEL, None, false);
    p.put(35, 16, "▲ VENTS", LABEL, None, false);
}

/// The tile a sled wears in the tray: its plate tinted by its kind, a lamp over it.
pub(super) fn tile(look: HardwareLook) -> Tile {
    let n = kind_colour(kind_of(look));
    Tile {
        bg: mix(n, K1, 0.78),
        ink: mix(n, rgb(0xffffff), 0.35),
        swatch: (n, K2),
    }
}
