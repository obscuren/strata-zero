//! Diegetic hardware in the city's register: a gunmetal unit with a hairline edge in
//! the district's accent, dark glass with cyan text, flat backlit keys. The keypad
//! and the terminal are built from it.

use crate::app::AppState;
use crate::render::Rgb;
use crate::ui::chrome::{CYAN, MAGENTA};
use citygen::DistrictId;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub const DEVICE_W: u16 = 25;
pub const DEVICE_H: u16 = 19;
/// The terminal: a unit around a 68×20 glass.
pub const CRT_W: u16 = 72;
pub const CRT_H: u16 = 24;
/// How much darker a scanline row is.
const SCANLINE: f32 = 0.88;

/// The unit's body.
pub const GUNMETAL: Color = Color::Rgb(40, 46, 54);
/// The glass, unlit.
pub const GLASS_BG: Color = Color::Rgb(6, 18, 22);
/// Live text on the glass.
pub const TEXT: Color = CYAN;
/// Refusals on the glass.
pub const DENY: Color = MAGENTA;
/// Quiet text on the glass: labels, hints.
pub const TEXT_DIM: Color = Color::Rgb(0, 120, 135);

/// The status light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Led {
    Idle,
    Ok,
    Fail,
}

/// How a key is lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyLit {
    Normal,
    Worn,
    Deny,
    Accept,
}

/// The accent a unit wears: its district's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skin {
    pub accent: Rgb,
}

impl Skin {
    pub fn for_district(st: &AppState, d: DistrictId) -> Skin {
        let accent = st
            .city
            .as_ref()
            .map(|c| st.style.theme(c.district(d).theme).accent)
            .unwrap_or(Rgb(0, 229, 255));
        Skin { accent }
    }

    /// The hairline edge: the accent, a little under full.
    pub fn edge(&self) -> Color {
        self.accent.scale(0.75).to_color()
    }

    fn led(&self, led: Led) -> Color {
        match led {
            Led::Idle => self.accent.scale(0.45).to_color(),
            Led::Ok => CYAN,
            Led::Fail => MAGENTA,
        }
    }

    /// A key's backlight and label.
    pub fn key_colours(&self, lit: KeyLit) -> (Color, Color) {
        let tint =
            |rgb: Rgb, bg: f32, fg: f32| (rgb.scale(bg).to_color(), rgb.scale(fg).to_color());
        match lit {
            KeyLit::Normal => tint(self.accent, 0.22, 1.0),
            KeyLit::Worn => tint(self.accent, 0.10, 0.45),
            KeyLit::Deny => tint(Rgb(255, 0, 200), 0.22, 1.0),
            KeyLit::Accept => tint(Rgb(0, 229, 255), 0.22, 1.0),
        }
    }
}

/// The unit: gunmetal, a heavy hairline edge in the accent, a badge and an LED.
pub fn bezel(buf: &mut Buffer, area: Rect, badge: &str, led: Led, skin: &Skin) {
    if area.width < 6 || area.height < 4 {
        return;
    }
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let c = &mut buf[(x, y)];
            c.reset();
            c.set_char(' ').set_bg(GUNMETAL);
        }
    }
    let edge = Style::default().fg(skin.edge()).bg(GUNMETAL);
    for x in area.x..area.right() {
        buf[(x, area.y)].set_char('━').set_style(edge);
        buf[(x, area.bottom() - 1)].set_char('━').set_style(edge);
    }
    for y in area.y..area.bottom() {
        buf[(area.x, y)].set_char('┃').set_style(edge);
        buf[(area.right() - 1, y)].set_char('┃').set_style(edge);
    }
    buf[(area.x, area.y)].set_char('┏').set_style(edge);
    buf[(area.right() - 1, area.y)]
        .set_char('┓')
        .set_style(edge);
    buf[(area.x, area.bottom() - 1)]
        .set_char('┗')
        .set_style(edge);
    buf[(area.right() - 1, area.bottom() - 1)]
        .set_char('┛')
        .set_style(edge);
    let label = Style::default()
        .fg(skin.accent.to_color())
        .bg(GUNMETAL)
        .add_modifier(Modifier::BOLD);
    let text = format!("▸ {badge}");
    for (i, ch) in text.chars().enumerate() {
        let x = area.x + 2 + i as u16;
        if x >= area.right() - 4 {
            break;
        }
        buf[(x, area.y + 1)].set_char(ch).set_style(label);
    }
    buf[(area.right() - 3, area.y + 1)]
        .set_char('●')
        .set_fg(skin.led(led))
        .set_bg(GUNMETAL);
}

/// The unit around a glass inset two cells; returns the glass rect.
pub fn crt(buf: &mut Buffer, area: Rect, badge: &str, led: Led, skin: &Skin) -> Rect {
    bezel(buf, area, badge, led, skin);
    let screen = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    for y in screen.y..screen.bottom() {
        for x in screen.x..screen.right() {
            buf[(x, y)].set_char(' ').set_bg(GLASS_BG);
        }
    }
    screen
}

/// A strip of glass on the unit: a thin edge, dark inside, lines of text.
pub fn glass(buf: &mut Buffer, area: Rect, lines: &[(String, Style)], skin: &Skin) {
    if area.width < 3 || area.height < 3 {
        return;
    }
    let edge = Style::default()
        .fg(skin.accent.scale(0.4).to_color())
        .bg(GUNMETAL);
    for x in area.x..area.right() {
        buf[(x, area.y)].set_char('─').set_style(edge);
        buf[(x, area.bottom() - 1)].set_char('─').set_style(edge);
    }
    for y in area.y..area.bottom() {
        buf[(area.x, y)].set_char('│').set_style(edge);
        buf[(area.right() - 1, y)].set_char('│').set_style(edge);
    }
    buf[(area.x, area.y)].set_char('┌').set_style(edge);
    buf[(area.right() - 1, area.y)]
        .set_char('┐')
        .set_style(edge);
    buf[(area.x, area.bottom() - 1)]
        .set_char('└')
        .set_style(edge);
    buf[(area.right() - 1, area.bottom() - 1)]
        .set_char('┘')
        .set_style(edge);
    for y in area.y + 1..area.bottom() - 1 {
        for x in area.x + 1..area.right() - 1 {
            buf[(x, y)].set_char(' ').set_bg(GLASS_BG);
        }
    }
    for (i, (text, style)) in lines.iter().enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.bottom() - 1 {
            break;
        }
        for (j, ch) in text.chars().enumerate() {
            let x = area.x + 2 + j as u16;
            if x >= area.right() - 1 {
                break;
            }
            buf[(x, y)].set_char(ch).set_style(style.bg(GLASS_BG));
        }
    }
}

/// One flat backlit key, 5 wide and a row tall, its label in the middle.
pub fn key(buf: &mut Buffer, x: u16, y: u16, label: char, lit: KeyLit, skin: &Skin) {
    let (bg, fg) = skin.key_colours(lit);
    for i in 0..5 {
        buf[(x + i, y)].set_char(' ').set_bg(bg);
    }
    buf[(x + 2, y)]
        .set_char(label)
        .set_style(Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD));
}

/// Darkens every other row of a glass a shade.
pub fn scanlines(buf: &mut Buffer, area: Rect) {
    for y in (area.y..area.bottom()).skip(1).step_by(2) {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            if let Color::Rgb(r, g, b) = cell.fg {
                let d = Rgb(r, g, b).scale(SCANLINE);
                cell.set_fg(d.to_color());
            }
        }
    }
}

/// Garbage scattered over the glass for a refusal, fading with `p` in 0..=1.
pub fn glitch(buf: &mut Buffer, area: Rect, tick: u64, p: f32) {
    for (x, y, ch) in crate::anim::effects::glitch_cells(area, tick, p) {
        if x < area.right() && y < area.bottom() {
            buf[(x, y)].set_char(ch).set_fg(DENY).set_bg(GLASS_BG);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    fn skin() -> Skin {
        Skin {
            accent: Rgb(255, 190, 0),
        }
    }

    #[test]
    fn the_unit_is_gunmetal_with_an_accent_edge_a_badge_and_a_led() {
        let area = Rect::new(2, 1, DEVICE_W, DEVICE_H);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        bezel(&mut buf, area, "SECURLOK", Led::Ok, &skin());
        assert_eq!(buf[(2, 1)].symbol(), "┏");
        assert_eq!(buf[(10, 1)].symbol(), "━");
        assert_eq!(buf[(10, 1)].fg, skin().edge());
        assert_eq!(buf[(2, 5)].symbol(), "┃");
        assert_eq!(buf[(area.right() - 1, area.bottom() - 1)].symbol(), "┛");
        assert_eq!(buf[(10, 10)].bg, GUNMETAL, "the body is gunmetal");
        let row: String = (area.x..area.right())
            .map(|x| buf[(x, 2)].symbol().to_string())
            .collect();
        assert!(row.contains("▸ SECURLOK"), "{row}");
        assert_eq!(buf[(area.right() - 3, 2)].symbol(), "●");
        assert_eq!(buf[(area.right() - 3, 2)].fg, CYAN);
        bezel(&mut buf, area, "SECURLOK", Led::Fail, &skin());
        assert_eq!(buf[(area.right() - 3, 2)].fg, MAGENTA);
        bezel(&mut buf, area, "SECURLOK", Led::Idle, &skin());
        assert_ne!(buf[(area.right() - 3, 2)].fg, MAGENTA);
    }

    #[test]
    fn the_glass_is_dark_with_cyan_text_and_a_key_is_a_backlit_cell() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        glass(
            &mut buf,
            Rect::new(4, 3, 21, 4),
            &[("[4][4][_][_]".to_string(), Style::default().fg(TEXT))],
            &skin(),
        );
        assert_eq!(buf[(4, 3)].symbol(), "┌");
        assert_eq!(buf[(6, 4)].symbol(), "[");
        assert_eq!(buf[(6, 4)].fg, TEXT);
        assert_eq!(buf[(6, 4)].bg, GLASS_BG);
        key(&mut buf, 10, 10, '7', KeyLit::Worn, &skin());
        let (bg, fg) = skin().key_colours(KeyLit::Worn);
        assert_eq!(buf[(10, 10)].bg, bg);
        assert_eq!(buf[(12, 10)].symbol(), "7");
        assert_eq!(buf[(12, 10)].fg, fg);
        let (nbg, nfg) = skin().key_colours(KeyLit::Normal);
        assert_ne!((bg, fg), (nbg, nfg), "worn keys are lit dimmer");
        let (dbg, _) = skin().key_colours(KeyLit::Deny);
        assert_ne!(dbg, nbg, "X carries its own tint");
    }

    #[test]
    fn the_crt_has_dark_glass_inside_the_unit_and_scanlines_dim_alternate_rows() {
        let area = Rect::new(3, 2, CRT_W, CRT_H);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        let screen = crt(&mut buf, area, "MUNI-OS", Led::Idle, &skin());
        assert_eq!(screen, Rect::new(5, 4, 68, 20));
        assert_eq!(buf[(5, 4)].bg, GLASS_BG);
        assert_eq!(buf[(72, 23)].bg, GLASS_BG);
        assert_eq!(buf[(3, 2)].symbol(), "┏");
        let cyan = Style::default().fg(TEXT).bg(GLASS_BG);
        buf[(10, 4)].set_char('A').set_style(cyan);
        buf[(10, 5)].set_char('B').set_style(cyan);
        scanlines(&mut buf, screen);
        assert_eq!(buf[(10, 4)].fg, TEXT);
        let Color::Rgb(r, g, b) = buf[(10, 5)].fg else {
            panic!("rgb")
        };
        let Color::Rgb(pr, pg, pb) = TEXT else {
            panic!()
        };
        assert!(
            r <= pr && g < pg && b < pb,
            "row 5 is darker: {:?}",
            buf[(10, 5)].fg
        );
        glitch(&mut buf, screen, 7, 0.0);
        let magenta = (screen.y..screen.bottom())
            .flat_map(|y| (screen.x..screen.right()).map(move |x| (x, y)))
            .filter(|(x, y)| buf[(*x, *y)].fg == DENY)
            .count();
        assert!(magenta > 0, "the glitch scatters magenta garbage");
    }
}
