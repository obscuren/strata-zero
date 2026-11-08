//! The Keyjack clipped on a door's pad: a black slab in a magenta case to the pad's
//! right, on a lead from the pad's edge. Its green LCD carries the bar (the track, the lit
//! zone, the marker), the beat and the lock's rating, the digits cracked so far and ERR,
//! FUSE BLOWN or OPEN; under it a light for every fault the run allows.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// A fault light that is spent.
pub const FAULT_LIT: Color = Color::Rgb(255, 40, 60);
/// One not spent yet.
const FAULT_DARK: Color = Color::Rgb(70, 20, 30);
/// The most lights the case has room for.
pub const MAX_LIGHTS: u8 = 6;
const BODY: Color = Color::Rgb(18, 16, 26);
const EDGE: Color = Color::Rgb(255, 0, 200);
const LCD: Color = Color::Rgb(4, 22, 12);
const ON: Color = Color::Rgb(90, 255, 150);
const DIM: Color = Color::Rgb(22, 90, 50);
const MARK: Color = Color::Rgb(230, 255, 235);
const INK: Color = Color::Rgb(150, 140, 170);
const RED: Color = Color::Rgb(255, 40, 60);
const RED_DK: Color = Color::Rgb(120, 20, 30);
const STATIC: Color = Color::Rgb(40, 50, 40);

/// What the LCD shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shows {
    /// The marker sweeps the bar.
    Sweep,
    /// A miss: the bar red and ERR, no marker.
    Err,
    /// The last fault burnt the fuse.
    Blown,
    /// The fourth hit opened the door.
    Open,
}

/// One frame of the run, as the slab draws it.
#[derive(Clone, Debug)]
pub struct Panel {
    pub shows: Shows,
    /// The bar's width in cells.
    pub bar: u16,
    /// The zone: its left cell and its width.
    pub zone: (u16, u16),
    /// The marker's cell while it sweeps.
    pub marker: u16,
    /// The digits cracked so far, left to right.
    pub digits: Vec<u8>,
    /// Faults run up on the door, and how many the run allows.
    pub faults: u8,
    pub allowed: u8,
    /// The beat it is on, 0 to 3.
    pub beat: usize,
    /// The lock's rating.
    pub rating: u8,
}

/// Where the slab hangs off a pad: three cells of lead to its right, two rows down; wide
/// enough for the bar and its caps inside the LCD.
pub fn rect(pad: Rect, bar: u16) -> Rect {
    Rect::new(pad.right() + 3, pad.y + 2, (bar + 8).max(26), 13)
}

fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, fg: Color, bg: Color, bold: bool) {
    let mut style = Style::default().fg(fg).bg(bg);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    buf.set_string(x, y, s, style);
}

fn fill(buf: &mut Buffer, r: Rect, bg: Color) {
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let c = &mut buf[(x, y)];
            c.reset();
            c.set_char(' ').set_bg(bg);
        }
    }
}

fn frame(buf: &mut Buffer, r: Rect, fg: Color, bg: Color, heavy: bool) {
    let (h, v, tl, tr, bl, br) = if heavy {
        ("━", "┃", "┏", "┓", "┗", "┛")
    } else {
        ("─", "│", "┌", "┐", "└", "┘")
    };
    for x in r.x + 1..r.right() - 1 {
        put(buf, x, r.y, h, fg, bg, false);
        put(buf, x, r.bottom() - 1, h, fg, bg, false);
    }
    for y in r.y + 1..r.bottom() - 1 {
        put(buf, r.x, y, v, fg, bg, false);
        put(buf, r.right() - 1, y, v, fg, bg, false);
    }
    put(buf, r.x, r.y, tl, fg, bg, false);
    put(buf, r.right() - 1, r.y, tr, fg, bg, false);
    put(buf, r.x, r.bottom() - 1, bl, fg, bg, false);
    put(buf, r.right() - 1, r.bottom() - 1, br, fg, bg, false);
}

/// The bar from `x`: its end caps, the track, the zone and the marker; red all over on
/// a miss.
fn bar(buf: &mut Buffer, x: u16, y: u16, p: &Panel, zone: Color, mark: Color) {
    let (at, w) = p.zone;
    let miss = p.shows == Shows::Err;
    put(buf, x, y, "▕", DIM, LCD, false);
    for i in 0..p.bar {
        let in_zone = i >= at && i < at + w;
        let (ch, fg) = if i == p.marker && p.shows == Shows::Sweep {
            ("█", mark)
        } else if in_zone {
            ("▒", if miss { RED } else { zone })
        } else {
            ("─", if miss { RED_DK } else { DIM })
        };
        put(buf, x + 1 + i, y, ch, fg, LCD, false);
    }
    put(buf, x + 1 + p.bar, y, "▏", DIM, LCD, false);
}

/// `4 7 _ _`: the digits cracked so far.
fn digits(p: &Panel) -> String {
    (0..4)
        .map(|i| match p.digits.get(i) {
            Some(d) => d.to_string(),
            None => "_".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The line under a pad for a runner who carries a Keyjack: `k  clip the keyjack`, or
/// greyed `k  keyjack · no fuse` when none has a fuse in.
pub fn clip_line(buf: &mut Buffer, pad: Rect, fused: bool) {
    let y = pad.bottom();
    if y >= buf.area.bottom() {
        return;
    }
    let (text, fg, key) = if fused {
        (" clip the keyjack ", Color::Rgb(205, 212, 222), EDGE)
    } else {
        (
            " keyjack · no fuse ",
            Color::Rgb(90, 96, 110),
            Color::Rgb(90, 60, 85),
        )
    };
    let w = 3 + text.chars().count() as u16;
    let x = pad.x + pad.width.saturating_sub(w) / 2;
    put(buf, x, y, " k ", key, BODY, true);
    put(buf, x + 3, y, text, fg, BODY, false);
}

/// Draws the slab beside `pad`, its lead clipped to the pad's right edge (`pad_body` is
/// the pad's case under the clip). Nothing when the screen has no room for it.
pub fn draw(buf: &mut Buffer, pad: Rect, pad_body: Color, p: &Panel) {
    let r = rect(pad, p.bar);
    if pad.width == 0 || r.right() > buf.area.right() || r.bottom() > buf.area.bottom() {
        return;
    }
    // The lead: clipped to the pad's edge, run to the slab.
    for x in pad.right()..r.x {
        put(
            buf,
            x,
            r.y + 4,
            "╍",
            Color::Rgb(170, 0, 130),
            Color::Rgb(10, 8, 14),
            false,
        );
    }
    put(buf, pad.right() - 1, r.y + 4, "╡", EDGE, pad_body, true);
    fill(buf, r, BODY);
    frame(buf, r, EDGE, BODY, true);
    put(buf, r.x, r.y + 4, "┫", EDGE, BODY, true);
    put(buf, r.x + 2, r.y + 1, "▞▚ KEYJACK", EDGE, BODY, true);
    put(
        buf,
        r.x + 2,
        r.y + 10,
        "└┬┘ ╶─┴─╴ ┌┴┐ ╶┬╴ └─┘",
        Color::Rgb(60, 30, 70),
        BODY,
        false,
    );
    put(
        buf,
        r.right() - 7,
        r.y + 2,
        "▚▞▚▞▚",
        Color::Rgb(70, 20, 70),
        BODY,
        false,
    );
    put(buf, r.x + 14, r.y + 1, "KJ-4", INK, BODY, false);
    let led = match p.shows {
        Shows::Open => Color::Rgb(80, 255, 140),
        Shows::Blown | Shows::Err => RED,
        Shows::Sweep => Color::Rgb(255, 180, 40),
    };
    put(buf, r.right() - 3, r.y + 1, "●", led, BODY, false);
    let glass = Rect::new(r.x + 2, r.y + 3, r.width - 4, 5);
    fill(buf, glass, LCD);
    frame(buf, glass, DIM, LCD, false);
    match p.shows {
        Shows::Blown => {
            put(
                buf,
                glass.x + 2,
                glass.y + 1,
                "▒▒░ ▒░▒▒░▒ ░▒▒▒░",
                STATIC,
                LCD,
                false,
            );
            put(buf, glass.x + 6, glass.y + 2, "FUSE  BLOWN", RED, LCD, true);
            put(
                buf,
                glass.x + 2,
                glass.y + 3,
                "░▒░▒▒ ░▒  ▒▒░▒░",
                STATIC,
                LCD,
                false,
            );
        }
        Shows::Open | Shows::Sweep | Shows::Err => {
            let (zone, mark) = if p.shows == Shows::Open {
                (ON, ON)
            } else {
                (ON, MARK)
            };
            bar(buf, glass.x + 1, glass.y + 1, p, zone, mark);
            put(buf, glass.x + 3, glass.y + 2, "BEAT", DIM, LCD, false);
            put(
                buf,
                glass.x + 8,
                glass.y + 2,
                &format!("{}/4  R{}", p.beat.min(3) + 1, p.rating),
                DIM,
                LCD,
                true,
            );
            put(buf, glass.x + 3, glass.y + 3, &digits(p), ON, LCD, true);
            match p.shows {
                Shows::Open => put(buf, glass.x + 13, glass.y + 3, "OPEN", ON, LCD, true),
                Shows::Err => put(buf, glass.x + 15, glass.y + 3, "ERR", RED, LCD, true),
                _ => {}
            }
        }
    }
    put(buf, r.x + 2, r.y + 9, "FAULT", INK, BODY, false);
    let allowed = p.allowed.min(MAX_LIGHTS);
    for i in 0..allowed {
        let lit = i < p.faults;
        let fg = if lit { FAULT_LIT } else { FAULT_DARK };
        put(buf, r.x + 8 + i as u16 * 2, r.y + 9, "●", fg, BODY, false);
    }
    put(buf, r.x + 2, r.y + 11, "space", EDGE, BODY, true);
    put(buf, r.x + 8, r.y + 11, "strike", INK, BODY, false);
    put(buf, r.x + 16, r.y + 11, "esc", EDGE, BODY, true);
    put(buf, r.x + 20, r.y + 11, "pull", INK, BODY, false);
}
