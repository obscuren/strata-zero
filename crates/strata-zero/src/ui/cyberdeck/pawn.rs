//! The pawn look: the deck drawn as a bench unit, 84 by 22. This holds where its
//! parts sit.

use super::{DeckView, GLASS_ROWS, GLASS_TEXT_W, GlassTone, Part, Pen, Switch};
use crate::ui::chrome::{CYAN, MAGENTA, PANEL_BG};
use ratatui::layout::Rect;
use ratatui::style::Color;

/// The drawing's width.
pub const W: u16 = 84;
/// The drawing's height.
pub const H: u16 = 22;
/// The row the inventory carts sit on.
pub const INV_Y: u16 = 19;
/// A cart in the inventory is this wide before its slots are counted.
const INV_BASE: u16 = 6;
/// Columns the inventory row may reach.
const INV_END: u16 = 81;

/// The pawn look's parts, relative to the drawing's origin.
pub fn parts(view: &DeckView) -> Vec<(Part, Rect)> {
    let mut out = Vec::new();
    // Bay: each loaded program spans its slots; the first empty slot is a stop too.
    let mut start = 0u16;
    for (k, cart) in view.bay.iter().enumerate() {
        let size = u16::from(cart.size.max(1));
        out.push((
            Part::Slot(k),
            Rect::new(8 + 14 * start, 1, 14 * size - 2, 3),
        ));
        start += size;
    }
    if start < u16::from(view.slots) {
        out.push((
            Part::Slot(view.bay.len()),
            Rect::new(8 + 14 * start, 1, 12, 3),
        ));
    }
    out.push((Part::Plate, Rect::new(4, 5, 15, 1)));
    let mut x = 9u16;
    for (k, rom) in view.roms.iter().enumerate() {
        let w = rom.chars().count() as u16 + 2;
        out.push((Part::Rom(k), Rect::new(x, 14, w, 1)));
        x += w + 1;
    }
    out.push((Part::Failsafe, Rect::new(69, 7, 10, 5)));
    out.extend(inventory(view));
    out
}

/// The inventory row on `INV_Y`: carts two apart from x 2, starting at `inv_offset`,
/// stopping once one would end past the edge.
fn inventory(view: &DeckView) -> Vec<(Part, Rect)> {
    let mut out = Vec::new();
    let mut x = 2u16;
    for (k, cart) in view.inventory.iter().enumerate().skip(view.inv_offset) {
        let w = INV_BASE + 6 * u16::from(cart.size.max(1));
        if x + w > INV_END {
            break;
        }
        out.push((Part::Inv(k), Rect::new(x, INV_Y, w, 2)));
        x += w + 2;
    }
    out
}

// The look's colours.
const BODY: Color = Color::Rgb(0x24, 0x21, 0x2a);
const BODY_HI: Color = Color::Rgb(0x32, 0x2e, 0x3a);
const BEZEL: Color = Color::Rgb(0x14, 0x12, 0x18);
const EDGE: Color = Color::Rgb(0xa0, 0x58, 0x3a);
const CART: Color = Color::Rgb(0x3b, 0x38, 0x42);
const CART_DIM: Color = Color::Rgb(0x2a, 0x28, 0x30);
const TAPE: Color = Color::Rgb(0xd9, 0xc9, 0xa0);
const LOOSE_TAPE: Color = Color::Rgb(0x8a, 0x82, 0x70);
const INK: Color = Color::Rgb(0x1a, 0x16, 0x12);
/// Exact black, not the terminal's: the skull sticker's ink, the ROM strips' text and
/// the hazard stripes' ground.
const BLACK: Color = Color::Rgb(0, 0, 0);
const GLASS: Color = Color::Rgb(0x14, 0x0e, 0x06);
const GLASS_LO: Color = Color::Rgb(0x10, 0x0b, 0x04);
const AMBER: Color = Color::Rgb(0xff, 0xb0, 0x00);
const AMBER_DIM: Color = Color::Rgb(0x8c, 0x60, 0x14);
const WARN: Color = Color::Rgb(0xff, 0x40, 0x50);
const HAZARD: Color = Color::Rgb(0xff, 0xcc, 0x00);
const COVER: Color = Color::Rgb(0x5a, 0x0c, 0x14);
const WELL: Color = Color::Rgb(0x0c, 0x0c, 0x10);
const RED: Color = Color::Rgb(0xff, 0x2a, 0x3c);
const HOT: Color = Color::Rgb(0xff, 0x6a, 0x00);
const GREEN: Color = Color::Rgb(0x50, 0xe6, 0x78);
const WHITE: Color = Color::Rgb(0xe8, 0xe8, 0xec);
const KEYS: Color = Color::Rgb(0x4c, 0x48, 0x55);
/// Labels on the body: bright for the bold ones, dull for the rest.
const LABEL: Color = Color::Rgb(0x8b, 0x85, 0x96);
const LABEL_DIM: Color = Color::Rgb(0x6b, 0x65, 0x75);
const EMPTY: Color = Color::Rgb(0x4a, 0x46, 0x52);
const LINK_OFF: Color = Color::Rgb(0x2a, 0x55, 0x60);
const HEAT_OFF: Color = Color::Rgb(0x4a, 0x30, 0x20);
const GHOST: Color = Color::Rgb(0x3c, 0x44, 0x50);

/// The maker plate and the stickers.
const MAKER: &str = " K▒ROK▒WA KX-2 ";
const STICKER: &str = " NO FUTURE ";
const SKULL: &str = " x‿x ";

/// The row the glass's bezel starts on.
const GLASS_Y: u16 = 6;
/// Where the tag starts beside the sticker, and the column it must stop before.
const TAG_X: u16 = 63;
const TAG_END: u16 = 68;
/// The failsafe frame's top-left cell.
const FX: u16 = 69;
const FY: u16 = 7;

/// Draws the pawn deck from the pen's origin, clipped to the pen's area.
pub(super) fn draw_with(view: &DeckView, p: &mut Pen) {
    p.fill(0, 0, W, H, PANEL_BG);
    bay(p, view);
    slab(p, view);
    glass(p, view);
    p.put(45, 13, SKULL, BLACK, Some(HAZARD), true);
    gauge(p, view);
    tag(p, view);
    failsafe(p, view.switch);
    keyboard(p);
    inventory_row(p, view);
    landing(p, view);
}

/// The carts standing in the bay and the empty slots.
fn bay(p: &mut Pen, view: &DeckView) {
    let mut start = 0u16;
    for (k, cart) in view.bay.iter().enumerate() {
        let size = u16::from(cart.size.max(1));
        let (x, w) = (8 + 14 * start, 14 * size - 2);
        let lifted = view.held.is_some() && view.point == Part::Slot(k);
        let (body, tape) = if lifted {
            (CART_DIM, LOOSE_TAPE)
        } else {
            (CART, TAPE)
        };
        let grip = format!("▗{}▖", "▄".repeat(usize::from(w) - 2));
        p.put(x, 1, &grip, body, None, false);
        p.fill(x, 2, w, 2, body);
        p.fill(x + 1, 2, w - 2, 2, tape);
        let name: String = cart.name.chars().take(usize::from(w) - 3).collect();
        p.put(x + 2, 2, &name, INK, Some(tape), true);
        let stats = format!("r{} · s{}", cart.rating, cart.size);
        p.put(x + 2, 3, &stats, INK, Some(tape), false);
        start += size;
    }
    // Every empty slot says so, though only the first is a stop.
    for i in start..u16::from(view.slots) {
        p.put(8 + 14 * i + 3, 3, "empty", EMPTY, None, false);
    }
}

/// The deck's body: edge, bay mouth, plate, lights, ROM row and sticker.
fn slab(p: &mut Pen, view: &DeckView) {
    p.fill(2, 5, 80, 12, BODY);
    p.frame((1, 4, 82, 14), EDGE, ['╭', '╮', '╰', '╯', '─', '│']);
    let mouth = "▀".repeat(14 * usize::from(view.slots));
    p.put(7, 4, &mouth, BEZEL, None, false);
    for i in 0..u16::from(view.slots) {
        let letter = char::from(b'A' + i as u8);
        p.put(
            7 + 14 * i,
            4,
            &format!(" {letter} "),
            LABEL_DIM,
            None,
            false,
        );
    }
    p.put(4, 5, MAKER, LABEL, Some(BODY_HI), true);
    let spec = format!("{} SLOT · R{}", view.slots, view.rating);
    p.put(20, 5, &spec, LABEL_DIM, Some(BODY), false);
    p.put(48, 5, "● PWR", GREEN, Some(BODY), false);
    let link = if view.link { CYAN } else { LINK_OFF };
    p.put(55, 5, "● LINK", link, Some(BODY), false);
    p.put(63, 5, "● HOT", HOT, Some(BODY), true);
    p.put(4, 14, "ROM▸", LABEL_DIM, Some(BODY), false);
    let strips = parts(view)
        .into_iter()
        .filter_map(|(q, r)| matches!(q, Part::Rom(_)).then_some(r));
    for (rom, r) in view.roms.iter().zip(strips) {
        p.put(r.x, r.y, &format!(" {rom} "), BLACK, Some(AMBER_DIM), false);
    }
    p.put(50, 10, STICKER, WHITE, Some(MAGENTA), true);
}

/// The glass: a bezel from `GLASS_Y`, amber text on dark amber, alternate rows darker,
/// the pinned line last. Characters past `typed` are not shown yet.
fn glass(p: &mut Pen, view: &DeckView) {
    p.frame((3, GLASS_Y, 42, 8), BEZEL, ['┏', '┓', '┗', '┛', '━', '┃']);
    p.fill(4, GLASS_Y + 1, 40, 6, GLASS);
    for r in (0..6).step_by(2) {
        p.tint(4, GLASS_Y + 1 + r, 40, 1, GLASS_LO);
    }
    let mut left = view.typed;
    let lines = view
        .glass
        .iter()
        .take(GLASS_ROWS)
        .enumerate()
        .map(|(k, line)| (k as u16, line))
        .chain([(5, &view.pinned)]);
    for (k, (text, tone)) in lines {
        let shown: String = text.chars().take(GLASS_TEXT_W).take(left).collect();
        left -= shown.chars().count();
        let (fg, bold) = match tone {
            GlassTone::Head => (AMBER, true),
            GlassTone::Body => (AMBER, false),
            GlassTone::Dim => (AMBER_DIM, false),
            GlassTone::Warn => (WARN, true),
        };
        p.put(5, GLASS_Y + 1 + k, &shown, fg, None, bold);
    }
    if view.more_above {
        p.put(43, GLASS_Y + 1, "▲", AMBER_DIM, None, false);
    }
    if view.more_below {
        p.put(43, GLASS_Y + GLASS_ROWS as u16, "▼", AMBER_DIM, None, false);
    }
}

/// HEAT: seven cells, one lit for each seventh of the trace begun.
fn gauge(p: &mut Pen, view: &DeckView) {
    let lit = view
        .heat
        .map_or(0, |h| (usize::from(h.min(100)) * 7).div_ceil(100));
    p.put(50, 7, "HEAT", LABEL, Some(BODY), true);
    p.put(50, 8, &"▮".repeat(lit), HOT, Some(BODY), false);
    let off = "▯".repeat(7 - lit);
    p.put(50 + lit as u16, 8, &off, HEAT_OFF, Some(BODY), false);
}

/// The player's handle, sprayed beside the sticker when it fits before the failsafe
/// frame, else under the sticker and cut to fit.
fn tag(p: &mut Pen, view: &DeckView) {
    let handle = view.handle.to_uppercase();
    if handle.chars().count() <= usize::from(TAG_END - TAG_X) {
        p.put(TAG_X, 10, &handle, CYAN, Some(BODY), true);
    } else {
        let cut: String = handle.chars().take(usize::from(TAG_END - 50)).collect();
        p.put(50, 12, &cut, CYAN, Some(BODY), true);
    }
}

/// The failsafe: a hazard frame round a well, the cover shut over it or flipped up.
fn failsafe(p: &mut Pen, switch: Switch) {
    let black = Some(BLACK);
    let stripes = "▞".repeat(10);
    p.put(FX, FY, &stripes, HAZARD, black, false);
    p.put(FX, FY + 4, &stripes, HAZARD, black, false);
    for r in 1..4 {
        p.put(FX, FY + r, "▞", HAZARD, black, false);
        p.put(FX + 9, FY + r, "▞", HAZARD, black, false);
    }
    p.put(FX + 1, FY + 5, "FAILSAFE", LABEL, Some(BODY), true);
    let lever = Color::Rgb(0xc8, 0xc8, 0xd0);
    if switch == Switch::On {
        let w = Some(COVER);
        p.fill(FX + 1, FY + 1, 8, 3, COVER);
        p.put(FX + 3, FY + 1, "▲", Color::Rgb(0xff, 0x80, 0x90), w, false);
        p.put(FX + 3, FY + 2, "█", lever, w, false);
        p.put(FX + 5, FY + 1, "ON", WHITE, w, true);
        p.put(
            FX + 5,
            FY + 3,
            "off",
            Color::Rgb(0xa0, 0x50, 0x60),
            w,
            false,
        );
        return;
    }
    let (w, dull) = (Some(WELL), Color::Rgb(0x5a, 0x5a, 0x66));
    p.put(FX + 1, FY - 1, &"▄".repeat(8), RED, Some(BODY), false);
    p.fill(FX + 1, FY + 1, 8, 3, WELL);
    if switch == Switch::Lifted {
        p.put(FX + 3, FY + 1, "▲", WHITE, w, false);
        p.put(FX + 3, FY + 2, "█", lever, w, false);
        p.put(FX + 5, FY + 1, "ON", GREEN, w, true);
        p.put(FX + 5, FY + 3, "off", dull, w, false);
    } else {
        p.put(FX + 3, FY + 2, "█", lever, w, false);
        p.put(FX + 3, FY + 3, "▼", RED, w, false);
        p.put(FX + 5, FY + 1, "on", dull, w, false);
        p.put(FX + 5, FY + 3, "OFF", RED, w, true);
    }
}

/// Two rows of keycaps, one missing and one replaced, and a space bar.
fn keyboard(p: &mut Pen) {
    let caps = "▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ··  ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆";
    p.put(5, 15, caps, KEYS, Some(BODY), false);
    p.put(20, 15, "▆▆", MAGENTA, Some(BODY), false);
    p.put(11, 16, &"▆".repeat(56), KEYS, Some(BODY), false);
}

/// The carts lying in the pack on `INV_Y` and the row under it, the label above them,
/// `‹` and `›` where more lie off either side, and the outline of a held cart's
/// landing in the bay.
fn inventory_row(p: &mut Pen, view: &DeckView) {
    p.put(2, INV_Y - 1, "INVENTORY", LABEL_DIM, None, true);
    let shown = inventory(view);
    for (part, r) in &shown {
        let Part::Inv(k) = part else { continue };
        let cart = &view.inventory[*k];
        if view.held.as_ref().is_some_and(|h| h.from == *k) {
            ghost(p, *r, GHOST, None);
            continue;
        }
        p.fill(r.x, r.y, r.width, 2, CART_DIM);
        p.fill(r.x + 1, r.y, r.width - 2, 2, LOOSE_TAPE);
        let name: String = cart.name.chars().take(usize::from(r.width) - 3).collect();
        p.put(r.x + 2, r.y, &name, INK, Some(LOOSE_TAPE), true);
        let stats = format!("r{} · s{}", cart.rating, cart.size);
        p.put(r.x + 2, r.y + 1, &stats, INK, Some(LOOSE_TAPE), false);
        p.put(r.x + r.width - 1, r.y, "▐", EMPTY, Some(CART_DIM), false);
    }
    if view.inv_offset > 0 {
        p.put(0, INV_Y, "‹", LABEL, None, true);
    }
    if view.inv_offset + shown.len() < view.inventory.len() {
        p.put(W - 1, INV_Y, "›", LABEL, None, true);
    }
}

/// The outline where the held cart would land, labelled with its name and slot.
fn landing(p: &mut Pen, view: &DeckView) {
    if let Some(held) = &view.held
        && matches!(view.point, Part::Slot(_))
    {
        let size = u16::from(held.cart.size.max(1));
        let r = Rect::new(8 + 14 * u16::from(held.start), 1, 14 * size - 2, 3);
        let label = format!("{} ▸ {}", held.cart.name, held.letter);
        ghost(p, r, CYAN, Some(&label));
    }
}

/// A dashed outline where a cart lies or would land.
fn ghost(p: &mut Pen, r: Rect, fg: Color, label: Option<&str>) {
    p.frame(
        (r.x, r.y, r.width, r.height),
        fg,
        ['┌', '┐', '└', '┘', '┄', '┆'],
    );
    if let Some(label) = label {
        let label: String = label.chars().take(usize::from(r.width) - 3).collect();
        p.put(r.x + 2, r.y + 1, &label, fg, None, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::cyberdeck::tests::{cart, kit_view};
    use crate::ui::cyberdeck::tests::{render, row, text};
    use crate::ui::cyberdeck::{GlassTone, Held, Switch, draw};
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier};

    const OX: u16 = 7;

    fn lit_kit() -> DeckView {
        let mut v = kit_view();
        v.glass = vec![
            ("SLOT A ▸ CRACK".into(), GlassTone::Head),
            (
                "rating 1 · size 1 · noise 4 · work 3".into(),
                GlassTone::Dim,
            ),
        ];
        v.pinned = ("[space] eject".into(), GlassTone::Body);
        v.handle = "VEX".into();
        v.heat = Some(60);
        v
    }

    fn count(s: &str, c: char) -> usize {
        s.chars().filter(|x| *x == c).count()
    }

    #[test]
    fn the_pawn_deck_draws_its_parts() {
        let buf = render(&lit_kit());
        let t = text(&buf);
        for want in [
            "K▒ROK▒WA KX-2",
            "2 SLOT · R1",
            "CRACK",
            "COPY",
            "r1 · s1",
            "FAILSAFE",
            "ON",
            "NETSCAN",
            "ARC TRACE",
            "INVENTORY",
            "ATTACK",
            "HEAT",
            "NO FUTURE",
            "x‿x",
            "VEX",
            "SLOT A ▸ CRACK",
            "[space] eject",
        ] {
            assert!(t.contains(want), "{want}");
        }
        for (x, y, glyph) in [
            (OX + 7, 0, "┏"),
            (OX + 20, 0, "┓"),
            (OX + 7, 4, "┗"),
            (OX + 20, 4, "┛"),
        ] {
            let cell = &buf[(x, y)];
            assert_eq!(cell.symbol(), glyph);
            let Color::Rgb(r, g, b) = cell.fg else {
                panic!("not rgb")
            };
            assert!(r == 0 && g > 150 && b > 150);
        }
    }

    #[test]
    fn heat_lights_a_cell_per_seventh_begun() {
        let mut v = lit_kit();
        let gauge = |v: &DeckView| {
            let r = row(&render(v), 8);
            (count(&r, '▮'), count(&r, '▯'))
        };
        assert_eq!(gauge(&v), (5, 2));
        v.heat = None;
        assert_eq!(gauge(&v), (0, 7));
        v.heat = Some(100);
        assert_eq!(gauge(&v), (7, 0));
        v.heat = Some(1);
        assert_eq!(gauge(&v), (1, 6));
    }

    #[test]
    fn the_failsafe_draws_three_ways() {
        let cover = Color::Rgb(0x5a, 0x0c, 0x14);
        let red = Color::Rgb(0xff, 0x2a, 0x3c);
        let mut v = lit_kit();
        let buf = render(&v);
        assert_eq!(buf[(OX + 70, 8)].bg, cover);
        assert_eq!(buf[(OX + 77, 10)].bg, cover);
        assert!(row(&buf, 8).contains("ON"));
        assert_eq!(count(&row(&buf, 6), '▄'), 0);

        v.switch = Switch::Lifted;
        let buf = render(&v);
        assert_eq!(count(&row(&buf, 6), '▄'), 8);
        for x in 70..78 {
            assert_eq!(buf[(OX + x, 6)].symbol(), "▄");
            assert_eq!(buf[(OX + x, 6)].fg, red);
        }
        assert_ne!(buf[(OX + 70, 8)].bg, cover);

        v.switch = Switch::Off;
        let buf = render(&v);
        let r10 = row(&buf, 10);
        assert!(r10.contains("OFF"));
        // The switch's label sits at x 74 of the drawing.
        for x in 74..77 {
            assert_eq!(buf[(OX + x, 10)].fg, red);
        }
        assert_ne!(buf[(OX + 70, 8)].bg, cover);
    }

    #[test]
    fn link_lights_when_jacked() {
        let mut v = lit_kit();
        let light = |v: &DeckView| {
            let buf = render(v);
            let r = row(&buf, 5);
            let at = r.find("LINK").unwrap();
            let x = r[..at].chars().count() as u16;
            buf[(x - 2, 5)].fg
        };
        let off = light(&v);
        v.link = true;
        let on = light(&v);
        assert_eq!(on, Color::Rgb(0, 229, 255));
        assert_ne!(off, on);
    }

    #[test]
    fn a_long_handle_moves_under_the_sticker() {
        let base = render(&lit_kit());
        let beside: String = row(&base, 10)
            .chars()
            .skip(OX as usize + 63)
            .take(3)
            .collect();
        assert_eq!(beside, "VEX");

        let mut v = lit_kit();
        v.handle = "D'Shawn".into();
        let buf = render(&v);
        let under: String = row(&buf, 12).chars().skip(OX as usize + 50).collect();
        assert!(under.starts_with("D'SHAWN"));
        assert!(!row(&buf, 10).contains("D'SHAWN"));
        for y in 7..12 {
            for x in 69..79 {
                assert_eq!(buf[(OX + x, y)], base[(OX + x, y)], "({x},{y})");
            }
        }

        v.handle = "ABCDEFGHIJKLMNOPQRST".into();
        let buf = render(&v);
        let r12 = row(&buf, 12);
        assert!(r12.contains("ABCDEFGHIJKLMNOPQR"));
        assert!(!r12.contains("ABCDEFGHIJKLMNOPQRS"));
        for y in 7..12 {
            for x in 69..79 {
                assert_eq!(buf[(OX + x, y)], base[(OX + x, y)], "({x},{y})");
            }
        }
    }

    #[test]
    fn odd_decks_draw() {
        let mut v = lit_kit();
        v.roms.clear();
        let buf = render(&v);
        let r14 = row(&buf, 14);
        assert!(r14.contains("ROM▸"));
        assert!(!r14.contains("NETSCAN"));
        assert_ne!(buf[(OX + 9, 14)].bg, Color::Rgb(0x8c, 0x60, 0x14));

        let mut v = lit_kit();
        v.bay = vec![cart("BACKDOOR", 2)];
        let buf = render(&v);
        let grip: String = (OX + 8..OX + 34).map(|x| buf[(x, 1)].symbol()).collect();
        assert_eq!(grip, format!("▗{}▖", "▄".repeat(24)));
        assert!(row(&buf, 2).contains("BACKDOOR"));
        assert!(row(&buf, 3).contains("r1 · s2"));
        assert!(!row(&buf, 3).contains("empty"));

        let mut v = lit_kit();
        v.slots = 4;
        v.point = Part::Failsafe;
        v.bay = vec![cart("CRACK", 1)];
        let buf = render(&v);
        let r4 = row(&buf, 4);
        for l in ["A", "B", "C", "D"] {
            assert!(r4.contains(&format!(" {l} ")), "{l}");
        }
        assert_eq!(row(&buf, 3).matches("empty").count(), 3);
        assert!(row(&buf, 5).contains("4 SLOT · R1"));
    }

    #[test]
    fn holding_draws_the_outline_where_it_lands() {
        let mut v = lit_kit();
        v.held = Some(Held {
            from: 0,
            cart: cart("SPOOF", 1),
            start: 1,
            letter: 'B',
        });
        v.point = Part::Slot(1);
        let buf = render(&v);
        assert!(row(&buf, 2).contains("SPOOF ▸ B"));
        assert_eq!(buf[(OX + 22, 1)].symbol(), "┌");
        assert_eq!(buf[(OX + 23, 1)].symbol(), "┄");
        assert_eq!(buf[(OX + 22, 3)].symbol(), "└");
        assert_eq!(buf[(OX + 23, 3)].symbol(), "┄");
        // The cart it came from lies there as a dim outline.
        assert_eq!(buf[(OX + 2, 19)].symbol(), "┌");
        assert_eq!(buf[(OX + 2, 20)].symbol(), "└");
        assert!(!row(&buf, 19).contains("ATTACK"));
    }

    #[test]
    fn inventory_overflow_shows_arrows() {
        let mut v = lit_kit();
        v.inventory = (0..8).map(|k| cart(&format!("P{k}"), 1)).collect();
        v.inv_offset = 2;
        v.point = Part::Inv(2);
        let buf = render(&v);
        assert_eq!(buf[(OX, 19)].symbol(), "‹");
        assert_eq!(buf[(OX + 83, 19)].symbol(), "›");
        assert!(row(&buf, 19).contains("P2"));
        assert!(!row(&buf, 19).contains("P1"));

        v.inv_offset = 0;
        let buf = render(&v);
        assert_ne!(buf[(OX, 19)].symbol(), "‹");
        assert_eq!(buf[(OX + 83, 19)].symbol(), "›");

        v.inventory.truncate(2);
        let buf = render(&v);
        assert_ne!(buf[(OX + 83, 19)].symbol(), "›");
    }

    #[test]
    fn glass_scroll_markers() {
        let mut v = lit_kit();
        v.more_below = true;
        let buf = render(&v);
        assert_eq!(buf[(OX + 43, 11)].symbol(), "▼");
        assert_ne!(buf[(OX + 43, 7)].symbol(), "▲");
        v.more_below = false;
        v.more_above = true;
        let buf = render(&v);
        assert_eq!(buf[(OX + 43, 7)].symbol(), "▲");
        assert_ne!(buf[(OX + 43, 11)].symbol(), "▼");
    }

    #[test]
    fn typed_cuts_the_glass() {
        let mut v = lit_kit();
        v.typed = 4;
        let t = text(&render(&v));
        assert!(t.contains("SLOT"));
        assert!(!t.contains("SLOT A"));
        assert!(!t.contains("[space] eject"));
        v.typed = 14;
        let t = text(&render(&v));
        assert!(t.contains("SLOT A ▸ CRACK"));
        assert!(!t.contains("rating"));
    }

    #[test]
    fn glass_tones_light_the_lines() {
        let mut v = lit_kit();
        v.glass = vec![
            ("head".into(), GlassTone::Head),
            ("body".into(), GlassTone::Body),
            ("dim".into(), GlassTone::Dim),
            ("warn".into(), GlassTone::Warn),
        ];
        let buf = render(&v);
        let at = |k: u16| &buf[(OX + 5, 7 + k)];
        assert_eq!(at(0).fg, Color::Rgb(0xff, 0xb0, 0x00));
        assert!(at(0).modifier.contains(Modifier::BOLD));
        assert!(!at(1).modifier.contains(Modifier::BOLD));
        assert_eq!(at(2).fg, Color::Rgb(0x8c, 0x60, 0x14));
        assert_eq!(at(3).fg, Color::Rgb(0xff, 0x40, 0x50));
        // Alternate rows are a shade darker.
        assert_ne!(at(0).bg, at(1).bg);
        assert_eq!(at(0).bg, at(2).bg);
    }

    #[test]
    fn every_glyph_is_single_width() {
        use unicode_width::UnicodeWidthStr;
        let mut v = lit_kit();
        v.held = Some(Held {
            from: 0,
            cart: cart("SPOOF", 1),
            start: 1,
            letter: 'B',
        });
        v.more_above = true;
        v.more_below = true;
        v.inventory = (0..8).map(|k| cart(&format!("P{k}"), 1)).collect();
        v.inv_offset = 2;
        for sw in [Switch::On, Switch::Lifted, Switch::Off] {
            v.switch = sw;
            let mut buf = Buffer::empty(Rect::new(0, 0, 98, 22));
            draw(&v, Rect::new(0, 0, 98, 22), &mut buf);
            for cell in &buf.content {
                assert_eq!(cell.symbol().width(), 1, "{:?}", cell.symbol());
            }
        }
    }

    #[test]
    fn every_colour_is_exact() {
        // A named colour follows the terminal's palette; the look's are its own.
        let mut v = lit_kit();
        v.more_above = true;
        v.more_below = true;
        let mut views: Vec<DeckView> = [Switch::On, Switch::Lifted, Switch::Off]
            .into_iter()
            .map(|switch| DeckView {
                switch,
                ..v.clone()
            })
            .collect();
        v.held = Some(Held {
            from: 0,
            cart: cart("SPOOF", 1),
            start: 1,
            letter: 'B',
        });
        v.point = Part::Slot(1);
        views.push(v);
        for v in &views {
            let buf = render(v);
            for y in 0..H {
                for x in OX..OX + W {
                    let c = &buf[(x, y)];
                    assert!(
                        matches!((c.fg, c.bg), (Color::Rgb(..), Color::Rgb(..))),
                        "{:?} {:?} ({x},{y}) {:?} on {:?}",
                        v.switch,
                        c.symbol(),
                        c.fg,
                        c.bg
                    );
                }
            }
        }
    }

    fn rect(parts: &[(Part, Rect)], p: Part) -> Option<Rect> {
        parts.iter().find(|(q, _)| *q == p).map(|(_, r)| *r)
    }

    #[test]
    fn parts_sit_where_the_spec_draws_them() {
        let p = parts(&kit_view());
        assert_eq!(rect(&p, Part::Slot(0)), Some(Rect::new(8, 1, 12, 3)));
        assert_eq!(rect(&p, Part::Slot(1)), Some(Rect::new(22, 1, 12, 3)));
        assert_eq!(rect(&p, Part::Slot(2)), None);
        assert_eq!(rect(&p, Part::Plate), Some(Rect::new(4, 5, 15, 1)));
        assert_eq!(rect(&p, Part::Rom(0)), Some(Rect::new(9, 14, 9, 1)));
        assert_eq!(rect(&p, Part::Rom(1)), Some(Rect::new(19, 14, 11, 1)));
        assert_eq!(rect(&p, Part::Failsafe), Some(Rect::new(69, 7, 10, 5)));
        assert_eq!(rect(&p, Part::Inv(0)), Some(Rect::new(2, 19, 12, 2)));

        let mut v = kit_view();
        v.bay = vec![cart("BACKDOOR", 2)];
        let p = parts(&v);
        assert_eq!(rect(&p, Part::Slot(0)), Some(Rect::new(8, 1, 26, 3)));
        assert_eq!(rect(&p, Part::Slot(1)), None);

        let mut v = kit_view();
        v.bay = vec![cart("CRACK", 1)];
        assert_eq!(
            rect(&parts(&v), Part::Slot(1)),
            Some(Rect::new(22, 1, 12, 3))
        );

        let mut v = kit_view();
        v.roms.clear();
        assert!(!parts(&v).iter().any(|(q, _)| matches!(q, Part::Rom(_))));

        let mut v = kit_view();
        v.slots = 4;
        v.point = Part::Failsafe;
        v.bay = vec![cart("CRACK", 1)];
        let empties: Vec<_> = parts(&v)
            .into_iter()
            .filter_map(|(q, _)| match q {
                Part::Slot(k) if k >= v.bay.len() => Some(k),
                _ => None,
            })
            .collect();
        assert_eq!(empties, vec![1]);
    }

    #[test]
    fn inventory_carts_lie_two_apart() {
        let mut v = kit_view();
        v.inventory = vec![cart("SPOOF", 1), cart("BACKDOOR", 2), cart("MASK", 1)];
        let p = parts(&v);
        let at = |k| rect(&p, Part::Inv(k)).map(|r| (r.x, r.width));
        assert_eq!(at(0), Some((2, 12)));
        assert_eq!(at(1), Some((16, 18)));
        assert_eq!(at(2), Some((36, 12)));

        v.inv_offset = 1;
        let p = parts(&v);
        assert_eq!(rect(&p, Part::Inv(0)), None);
        assert_eq!(rect(&p, Part::Inv(1)).map(|r| r.x), Some(2));
    }
}
