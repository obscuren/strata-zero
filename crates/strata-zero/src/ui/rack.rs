//! The rig drawn as its rack, made from the frame and its fitted sleds: two rails, a
//! head unit with a little green LCD reading the frame's head line, one sled per bay
//! (a dark faceplate with a pull handle at the left, vents at the right, its kind and
//! model on the plate and its tier in a row of lamps along the bottom, all in the
//! kind's colour), a dim blanking plate stamped with its kind for an empty bay, the
//! frame's base under it (the Zenbo's milk crate, a cabinet's plinth, a tower's foot,
//! a server frame's slide rails) and the loom running off to the wall at the right.
//!
//! The drawing fits the area it is given: the sleds shrink from four cell rows of
//! plate to one, and the head unit and base thin, before anything is left off.
//! Pixels are half cells, folded into half blocks; text goes over the folded cells.

use crate::app::AppState;
use crate::ui::cyberdeck::Pen;
use crate::ui::hardware::rgb;
use crate::ui::hardware::sled::{K0, K1, K2, K3, ST1, ST2, ST3, kind_colour, mix, tier, unlit};
use crate::ui::pixels::Pixels;
use game::{FrameBase, ItemId, Mod, SocketKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// The rack at its widest, in cells, loom included.
pub const WIDTH: u16 = 58;
/// The head unit's and the base's rows at full size.
const HEAD: u16 = 3;
const BASE: u16 = 4;
/// Rows per sled, head rows and base rows, largest first: the sleds give way first,
/// then the head unit and base, and a bay is never dropped.
const FITS: [(u16, u16, u16); 9] = [
    (4, HEAD, BASE),
    (3, HEAD, BASE),
    (3, HEAD, 3),
    (2, HEAD, 3),
    (2, HEAD, 2),
    (2, 2, 2),
    (1, 2, 2),
    (1, 2, 1),
    (1, 1, 1),
];

const LCD: Color = rgb(0x081410);
const LCD_TEXT: Color = rgb(0x50e678);
const AMBER: Color = rgb(0xffb000);
/// A plate on a one-row sled every other bay, so neighbours read apart.
const K2_ALT: Color = rgb(0x26242c);
/// The loom's cords, bay by bay.
const LOOM: [Color; 5] = [
    rgb(0x34323b),
    ST2,
    rgb(0x2a6a7a),
    rgb(0x5a2a4a),
    rgb(0x4a4630),
];

/// The rack the desk draws: the frame, its bays top to bottom with the sled in each,
/// and the part picked at the desk (`None` for the frame itself).
#[derive(Clone, Debug, PartialEq)]
pub struct RackArt {
    pub frame: ItemId,
    pub bays: Vec<(SocketKind, Option<ItemId>)>,
    pub picked: Option<usize>,
}

impl RackArt {
    /// The player's rig as it stands.
    pub fn rig(st: &AppState, picked: Option<usize>) -> RackArt {
        RackArt {
            frame: st.character.rig_frame(),
            bays: st.character.rig_fitted(&st.content),
            picked,
        }
    }
}

/// The rack's size in cells with `rows` cell rows to each of `bays` sleds, at full
/// width with the full head unit and base.
pub fn size(bays: usize, rows: u16) -> (u16, u16) {
    (WIDTH, HEAD + bays as u16 * rows + BASE)
}

/// A sled's short model for its plate: its name without the trailing words in lower
/// case that say what it is ("ZB-8 core" is `ZB-8`, "CU-1 copper line" `CU-1`),
/// upper case.
pub(crate) fn model(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let keep = words
        .iter()
        .rposition(|w| w.chars().any(|c| !c.is_lowercase()))
        .map_or(words.len(), |i| i + 1);
    words[..keep].join(" ").to_uppercase()
}

/// How a frame's rails, head unit and base are coloured.
struct Body {
    rail: Color,
    rail_hi: Color,
    head: Color,
    head_hi: Color,
}

fn body(base: FrameBase) -> Body {
    match base {
        FrameBase::Crate => Body {
            rail: ST1,
            rail_hi: ST2,
            head: K2,
            head_hi: K3,
        },
        FrameBase::Plinth => Body {
            rail: rgb(0x323a44),
            rail_hi: rgb(0x4a545f),
            head: rgb(0x262c34),
            head_hi: rgb(0x3a424c),
        },
        FrameBase::Foot => Body {
            rail: rgb(0x2a1434),
            rail_hi: rgb(0xff4fd8),
            head: rgb(0x1e0c26),
            head_hi: rgb(0x3a1a46),
        },
        FrameBase::Rails => Body {
            rail: rgb(0xaeb6c0),
            rail_hi: rgb(0xe6ecf2),
            head: rgb(0xc8ced6),
            head_hi: rgb(0xeef2f6),
        },
    }
}

/// A line of pixels from (x0, y0) to (x1, y1), ends included.
fn line(c: &mut Pixels, (mut x0, mut y0): (i32, i32), (x1, y1): (i32, i32), col: Color) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    let mut err = dx + dy;
    loop {
        c.px(x0, y0, Some(col));
        if (x0, y0) == (x1, y1) {
            return;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// What a bay shows: its kind's colour, and the fitted sled's model and tier.
struct Bay {
    kind: SocketKind,
    n: Color,
    sled: Option<(String, u8)>,
}

fn bays(art: &RackArt, st: &AppState) -> Vec<Bay> {
    art.bays
        .iter()
        .map(|(kind, sled)| Bay {
            kind: *kind,
            n: kind_colour(*kind),
            sled: sled.as_ref().map(|id| {
                let item = st.content.item(id);
                let mods: &[Mod] = item
                    .and_then(|i| i.hardware.as_ref())
                    .map_or(&[], |h| h.mods.as_slice());
                (
                    model(item.map_or(id.0.as_str(), |i| i.name.as_str())),
                    tier(mods),
                )
            }),
        })
        .collect()
}

/// Draws the rack into `area`, centred across it and standing on its foot, as large
/// as fits. Returns the whole rack's cells first, then each bay's plate, top to
/// bottom, each clipped to the area.
pub fn draw(art: &RackArt, st: &AppState, area: Rect, buf: &mut Buffer) -> Vec<Rect> {
    let n = art.bays.len() as u16;
    if area.width < 24 || area.height == 0 {
        let none = Rect::new(area.x, area.y, 0, 0);
        return vec![none; 1 + art.bays.len()];
    }
    let (r, hr, br) = FITS
        .into_iter()
        .find(|&(r, hr, br)| hr + n * r + br <= area.height)
        .unwrap_or(FITS[FITS.len() - 1]);
    let w = area.width.min(WIDTH);
    let h = hr + n * r + br;
    let ox = area.x + (area.width - w) / 2;
    let oy = area.y + area.height.saturating_sub(h);
    let spec = st.content.item(&art.frame).and_then(|i| i.frame.as_ref());
    let head_line = spec.map_or_else(
        || {
            st.content
                .item(&art.frame)
                .map_or(String::new(), |i| i.name.to_uppercase())
        },
        |f| f.head.clone(),
    );
    let base = spec.map_or(FrameBase::Crate, |f| f.base);
    let b = body(base);
    let bays = bays(art, st);

    let (wi, hi) = (i32::from(w), i32::from(h));
    let (pl, pr) = (4, wi - 9);
    let (hr_i, r_i) = (i32::from(hr), i32::from(r));
    let sleds_end = 2 * (hr_i + i32::from(n) * r_i);
    let mut c = Pixels::new(w, h);
    // The rack's dark inside, then its rails with their holes.
    c.rect(pl, 0, pr, sleds_end, K0);
    for (x0, lit) in [(1, 1), (pr, pr + 2)] {
        c.rect(x0, 0, x0 + 3, sleds_end, b.rail);
        c.vline(lit, 0, sleds_end, b.rail_hi);
        for y in (2..sleds_end).step_by(3) {
            c.px(x0 + 1, y, Some(K0));
        }
    }
    // The head unit: its LCD at the left, a lamp and a knob at the right.
    let (hy0, hy1, lcd_row) = match hr {
        1 => (0, 2, 0),
        2 => (0, 3, 0),
        _ => (1, 2 * hr_i - 1, 1),
    };
    c.rect(pl, hy0, pr, hy1, b.head);
    c.hline(pl, pr, hy0, b.head_hi);
    let lcd_w = (head_line.chars().count() as i32 + 2).min(pr - pl - 10);
    c.rect(pl + 2, 2 * lcd_row, pl + 2 + lcd_w, 2 * lcd_row + 2, LCD);
    c.rect(pr - 4, 2 * lcd_row, pr - 2, 2 * lcd_row + 2, AMBER);
    c.vline(pr - 7, hy0 + 1, hy1, ST2);
    // The sleds, or a blanking plate where a bay is empty.
    for (i, bay) in bays.iter().enumerate() {
        let y0 = hr_i + i as i32 * r_i;
        if r < 2 {
            continue;
        }
        let (py0, py1) = (2 * y0, 2 * (y0 + r_i) - 1);
        let Some((name, lit)) = &bay.sled else {
            c.rect(pl, py0, pr, py1, K1);
            if r >= 3 {
                c.hline(pl, pr, py0, K2);
            }
            for x in [pl + 1, pr - 2] {
                c.px(x, py0 + 1, Some(ST1));
                c.px(x, py1 - 1, Some(ST1));
            }
            continue;
        };
        // A two-row plate carries its writing on its top row, so only a taller one
        // has room for the lit edge.
        c.rect(pl, py0, pr, py1, K2);
        if r >= 3 {
            c.hline(pl, pr, py0, K3);
        }
        // The pull handle, with its grip hole on a taller plate.
        c.rect(pl + 1, py0 + 1, pl + 4, py1, ST2);
        c.vline(pl + 1, py0 + 1, py1, ST3);
        if r >= 3 {
            c.vline(pl + 2, py0 + 2, py1 - 1, K0);
        }
        // The lamps along the foot of the plate, the tier's pairs lit.
        for pair in 0..3u8 {
            for k in 0..2 {
                let x = pl + 6 + i32::from(pair) * 5 + k * 2;
                let col = if pair < *lit { bay.n } else { unlit(bay.n) };
                c.px(x, py1 - 1, Some(col));
            }
        }
        // Vents, right of the writing.
        let text_end = pl + 6 + (bay.kind.label().len() + 2 + name.chars().count()) as i32;
        let (vy0, vy1) = if r == 2 {
            (py0 + 1, py1)
        } else {
            (py0 + 2, py1 - 1)
        };
        for x in ((text_end + 2).max(pr - 12)..pr - 1).step_by(2) {
            c.vline(x, vy0, vy1, K0);
        }
    }
    draw_base(&mut c, base, sleds_end, 2 * hi, wi - 5);
    // The loom, a cord from each fitted sled off to the wall.
    for (i, bay) in bays.iter().enumerate() {
        if bay.sled.is_none() {
            continue;
        }
        let y = 2 * (hr_i + i as i32 * r_i) + r_i;
        let col = LOOM[i % LOOM.len()];
        let pts = [
            (wi - 6, y),
            (wi - 4, y + 3),
            (wi - 2, y + 10),
            (wi - 1, y + 14),
        ];
        for pair in pts.windows(2) {
            line(&mut c, pair[0], pair[1], col);
        }
    }

    let mut p = Pen {
        clip: area.intersection(buf.area),
        buf,
        ox,
        oy,
    };
    c.fold(&mut p, 0, 0);
    // Writing: the head line on the LCD, each plate's kind and model.
    let room = (lcd_w - 2).max(0) as usize;
    let shown: String = head_line.chars().take(room).collect();
    p.put(
        pl as u16 + 3,
        lcd_row as u16,
        &shown,
        LCD_TEXT,
        Some(LCD),
        false,
    );
    let (pl, pr) = (pl as u16, pr as u16);
    for (i, bay) in bays.iter().enumerate() {
        let y0 = hr + i as u16 * r;
        let ty = y0 + u16::from(r >= 3);
        let label = bay.kind.label().to_uppercase();
        let plate = if r == 1 && i % 2 == 1 { K2_ALT } else { K2 };
        match &bay.sled {
            None => {
                if r == 1 {
                    p.fill(pl, y0, pr - pl, 1, K1);
                }
                let stamp = format!("── {label} ──");
                let x = pl + (pr - pl).saturating_sub(stamp.chars().count() as u16) / 2;
                p.put(x, ty, &stamp, mix(bay.n, K1, 0.6), Some(K1), false);
            }
            Some((name, lit)) => {
                if r == 1 {
                    p.fill(pl, y0, pr - pl, 1, plate);
                    p.cell(i32::from(pl) + 1, i32::from(y0), '▐', ST3, Some(plate));
                    p.cell(i32::from(pl) + 2, i32::from(y0), '▌', ST2, Some(plate));
                    let end = pl + 6 + (label.len() + 2 + name.chars().count()) as u16;
                    let x0 = pr.saturating_sub(10);
                    if x0 > end + 1 {
                        for pair in 0..3u8 {
                            for k in 0..2 {
                                let x = x0 + u16::from(pair) * 3 + k;
                                let col = if pair < *lit { bay.n } else { unlit(bay.n) };
                                p.put(x, y0, "▪", col, Some(plate), false);
                            }
                        }
                    }
                }
                let bg = if r == 1 { plate } else { K2 };
                p.put(pl + 6, ty, &label, bay.n, Some(bg), true);
                let at = pl + 8 + label.len() as u16;
                p.put(at, ty, name, mix(bay.n, K2, 0.45), Some(bg), false);
            }
        }
    }

    let clip = |rect: Rect| area.intersection(rect);
    let mut out = vec![clip(Rect::new(ox, oy, w - 5, h))];
    for i in 0..n {
        out.push(clip(Rect::new(ox + pl, oy + hr + i * r, pr - pl, r)));
    }
    out
}

/// The frame's base, pixel rows `y0..y1` and columns `0..x1`.
fn draw_base(c: &mut Pixels, base: FrameBase, y0: i32, y1: i32, x1: i32) {
    if y1 <= y0 {
        return;
    }
    match base {
        // A blue milk crate, its grid of holes.
        FrameBase::Crate => {
            let (blue, hi, lo) = (rgb(0x1d3d6b), rgb(0x2b5590), rgb(0x0f2340));
            c.rect(0, y0, x1, y1, blue);
            c.hline(0, x1, y0, hi);
            for y in (y0 + 2..y1 - 2).step_by(3) {
                for x in (2..x1 - 2).step_by(4) {
                    c.rect(x, y, x + 2, y + 2, lo);
                }
            }
            if y1 - y0 > 2 {
                c.hline(0, x1, y1 - 1, lo);
            }
        }
        // A ward office's plinth, a kick strip and the carpet still stuck to it.
        FrameBase::Plinth => {
            let (plinth, hi) = (rgb(0x2a3038), rgb(0x3e4650));
            let (rug, rug2) = (rgb(0x5c3b2c), rgb(0x6e4a36));
            c.rect(0, y0, x1, y1, plinth);
            c.hline(0, x1, y0, hi);
            if y1 - y0 > 3 {
                c.hline(2, x1 - 2, y1 - 3, K0);
            }
            for x in 0..x1 {
                c.px(x, y1 - 1, Some(if x % 3 == 0 { rug2 } else { rug }));
            }
        }
        // A tower's pedestal: a neck down to a wide foot with a neon seam.
        FrameBase::Foot => {
            let (dark, neon) = (rgb(0x220c2c), rgb(0xff4fd8));
            let mid = x1 / 2;
            let foot = (y1 - 2).max(y0);
            c.rect(mid - 8, y0, mid + 8, foot, dark);
            c.vline(mid - 8, y0, foot, mix(neon, dark, 0.5));
            c.vline(mid + 7, y0, foot, mix(neon, dark, 0.5));
            c.rect((mid - 18).max(0), foot, (mid + 18).min(x1), y1, dark);
            c.hline((mid - 18).max(0), (mid + 18).min(x1), foot, neon);
        }
        // A server frame's slide rails, on rubber feet.
        FrameBase::Rails => {
            c.hline(0, x1, y0, ST3);
            c.hline(0, x1, y0 + 1, ST2);
            for x in (2..x1).step_by(4) {
                c.px(x, y0 + 1, Some(K0));
            }
            for x0 in [3, x1 - 7] {
                c.rect(x0, y0 + 2, x0 + 4, y1, ST1);
                c.hline(x0, x0 + 4, y1 - 1, K0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::buffer_text;
    use crate::ui::hardware::rgb;
    use crate::ui::hardware::sled::kind_colour;
    use ratatui::style::Color;

    /// The rack drawn into `w` by `h` cells, and its rects.
    fn drawn(st: &AppState, art: &RackArt, w: u16, h: u16) -> (Buffer, Vec<Rect>) {
        let mut buf = Buffer::empty(Rect::new(0, 0, w + 4, h + 2));
        let rects = draw(art, st, Rect::new(2, 1, w, h), &mut buf);
        (buf, rects)
    }

    /// Where `text` starts in the buffer.
    fn find(b: &Buffer, text: &str) -> Option<(u16, u16)> {
        let n = text.chars().count() as u16;
        (0..b.area.height).find_map(|y| {
            (0..b.area.width.saturating_sub(n - 1)).find_map(|x| {
                let s: String = (x..x + n).map(|x| b[(x, y)].symbol()).collect();
                (s == text).then_some((x, y))
            })
        })
    }

    fn ink(b: &Buffer, text: &str) -> Color {
        let (x, y) = find(b, text).unwrap_or_else(|| panic!("{text}:\n{}", buffer_text(b)));
        b[(x, y)].fg
    }

    #[test]
    fn each_bay_is_a_sled_or_a_plate() {
        let st = AppState::new();
        let mut art = RackArt::rig(&st, None);
        let (b, _) = drawn(&st, &art, 78, 28);
        for kind in [SocketKind::Core, SocketKind::Relay, SocketKind::Line] {
            let label = kind.label().to_uppercase();
            assert_eq!(ink(&b, &label), kind_colour(kind), "{label}");
        }
        assert!(find(&b, "ZB-8").is_some() && find(&b, "R-3").is_some());
        assert!(find(&b, "CU-1").is_some(), "{}", buffer_text(&b));
        // Pull the relay: a blanking plate stamped with its kind, dim.
        art.bays[1].1 = None;
        let (b, _) = drawn(&st, &art, 78, 28);
        let plate = ink(&b, "── RELAY ──");
        assert_ne!(plate, kind_colour(SocketKind::Relay));
        let Color::Rgb(r, g, bl) = plate else {
            panic!("{plate:?}")
        };
        assert!(u16::from(r) + u16::from(g) + u16::from(bl) < 300, "dim");
        assert!(find(&b, "R-3").is_none());
    }

    #[test]
    fn the_head_unit_reads_its_line() {
        let st = AppState::new();
        let (b, _) = drawn(&st, &RackArt::rig(&st, None), 78, 28);
        assert_eq!(ink(&b, "ZENBO 64  REDY"), rgb(0x50e678));
    }

    #[test]
    fn a_halcyon_has_five_bays() {
        let mut st = AppState::new();
        let halcyon = ItemId::new("rack_halcyon");
        st.character.set_frame(&st.content, &halcyon);
        let art = RackArt::rig(&st, Some(0));
        assert_eq!(art.frame, halcyon);
        let (b, rects) = drawn(&st, &art, 78, 28);
        assert_eq!(rects.len(), 6);
        assert!(find(&b, "HALCYON NODE 9  OK").is_some());
        assert!(
            find(&b, "── FIREWALL ──").is_some(),
            "its firewall bay is empty"
        );
        for (i, r) in rects[1..].iter().enumerate() {
            assert!(r.height >= 1 && rects[0].contains(r.as_position()), "{i}");
            for s in &rects[i + 2..] {
                assert!(!r.intersects(*s), "{r:?} {s:?}");
            }
        }
    }

    #[test]
    fn it_fits_100x30() {
        let mut st = AppState::new();
        for frame in ["rack_zenbo", "rack_halcyon"] {
            st.character.set_frame(&st.content, &ItemId::new(frame));
            let art = RackArt::rig(&st, None);
            let bays = art.bays.len();
            for (w, h) in [(78, 28), (58, 24), (58, 18), (58, 12)] {
                let area = Rect::new(2, 1, w, h);
                let (b, rects) = drawn(&st, &art, w, h);
                assert_eq!(rects.len(), 1 + bays, "{frame} {w}x{h}");
                for r in &rects {
                    assert!(r.height >= 1, "{frame} {w}x{h}: {r:?}");
                    assert_eq!(area.intersection(*r), *r, "{frame} {w}x{h}: {r:?}");
                }
                // Every bay keeps its kind on the plate, however small.
                assert!(find(&b, "CORE").is_some(), "{frame} {w}x{h}");
                assert!(find(&b, "LINE").is_some(), "{frame} {w}x{h}");
                // Nothing drawn outside the area.
                for y in 0..b.area.height {
                    for x in 0..b.area.width {
                        if !area.contains((x, y).into()) {
                            assert_eq!(b[(x, y)], ratatui::buffer::Cell::EMPTY);
                        }
                    }
                }
            }
        }
        let (w, h) = size(5, 3);
        assert!(w <= 58 && h <= 24, "{w}x{h}");
        assert!(size(3, 4).1 < size(5, 4).1);
    }

    #[test]
    fn the_short_model_drops_the_kind() {
        assert_eq!(model("ZB-8 core"), "ZB-8");
        assert_eq!(model("Tinman 2X core"), "TINMAN 2X");
        assert_eq!(model("CU-1 copper line"), "CU-1");
        assert_eq!(model("Longwire"), "LONGWIRE");
        assert_eq!(model("Sandbag firewall"), "SANDBAG");
    }
}
