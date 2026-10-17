//! Diegetic hardware: a moulded beige bezel, a recessed LCD and raised keycaps,
//! drawn in truecolor cells. The keypad is built from them; the terminal frame
//! will be.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

pub const DEVICE_W: u16 = 25;
pub const DEVICE_H: u16 = 19;

pub const BEIGE_LIGHT: Color = Color::Rgb(226, 214, 184);
pub const BEIGE: Color = Color::Rgb(196, 184, 152);
pub const BEIGE_DARK: Color = Color::Rgb(120, 108, 84);
pub const INK: Color = Color::Rgb(40, 36, 30);
pub const INK_WORN: Color = Color::Rgb(128, 118, 98);
pub const PHOSPHOR: Color = Color::Rgb(120, 255, 140);
pub const LCD_BG: Color = Color::Rgb(8, 24, 12);
pub const LED_AMBER: Color = Color::Rgb(255, 190, 60);
pub const LED_GREEN: Color = Color::Rgb(80, 255, 110);
pub const LED_RED: Color = Color::Rgb(255, 60, 60);

/// The status light on a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Led {
    Amber,
    Green,
    Red,
}

impl Led {
    fn colour(self) -> Color {
        match self {
            Led::Amber => LED_AMBER,
            Led::Green => LED_GREEN,
            Led::Red => LED_RED,
        }
    }
}

/// The moulded frame: a light top edge, mid sides, a dark bottom edge, the inside
/// filled; a badge top left and an LED top right.
pub fn bezel(buf: &mut Buffer, area: Rect, badge: &str, led: Led) {
    if area.width < 6 || area.height < 4 {
        return;
    }
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let c = &mut buf[(x, y)];
            c.reset();
            c.set_char(' ').set_bg(BEIGE);
        }
    }
    for x in area.x..area.right() {
        buf[(x, area.y)]
            .set_char('▄')
            .set_fg(BEIGE_LIGHT)
            .set_bg(Color::Reset);
        buf[(x, area.bottom() - 1)]
            .set_char('▀')
            .set_fg(BEIGE_DARK)
            .set_bg(Color::Reset);
    }
    for y in area.y + 1..area.bottom() - 1 {
        buf[(area.x, y)]
            .set_char('▐')
            .set_fg(BEIGE_LIGHT)
            .set_bg(BEIGE);
        buf[(area.right() - 1, y)]
            .set_char('▌')
            .set_fg(BEIGE_DARK)
            .set_bg(BEIGE);
    }
    let ink = Style::default().fg(INK).bg(BEIGE);
    for (i, ch) in badge.chars().enumerate() {
        let x = area.x + 2 + i as u16;
        if x >= area.right() - 4 {
            break;
        }
        buf[(x, area.y + 1)].set_char(ch).set_style(ink);
    }
    buf[(area.right() - 3, area.y + 1)]
        .set_char('●')
        .set_fg(led.colour())
        .set_bg(BEIGE);
}

/// The recessed screen: a thin box on the bezel, dark glass inside, lines of text.
pub fn lcd(buf: &mut Buffer, area: Rect, lines: &[(String, Style)]) {
    if area.width < 3 || area.height < 3 {
        return;
    }
    let edge = Style::default().fg(BEIGE_DARK).bg(BEIGE);
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
            buf[(x, y)].set_char(' ').set_bg(LCD_BG);
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
            buf[(x, y)].set_char(ch).set_style(style.bg(LCD_BG));
        }
    }
}

/// One raised key, 5 wide and 3 tall, its label in the middle.
pub fn keycap(buf: &mut Buffer, x: u16, y: u16, label: char, ink: Style) {
    let edge = Style::default().fg(BEIGE_LIGHT).bg(BEIGE);
    let shade = Style::default().fg(BEIGE_DARK).bg(BEIGE);
    let top = ['┌', '─', '─', '─', '┐'];
    let mid = ['│', ' ', label, ' ', '│'];
    let bot = ['└', '─', '─', '─', '┘'];
    for (i, ch) in top.iter().enumerate() {
        buf[(x + i as u16, y)].set_char(*ch).set_style(edge);
    }
    for (i, ch) in mid.iter().enumerate() {
        let style = match i {
            0 => edge,
            4 => shade,
            2 => ink.bg(BEIGE),
            _ => Style::default().bg(BEIGE),
        };
        buf[(x + i as u16, y + 1)].set_char(*ch).set_style(style);
    }
    for (i, ch) in bot.iter().enumerate() {
        buf[(x + i as u16, y + 2)].set_char(*ch).set_style(shade);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn the_bezel_is_beige_with_a_badge_and_a_coloured_led() {
        let area = Rect::new(2, 1, DEVICE_W, DEVICE_H);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        bezel(&mut buf, area, "SECURLOK", Led::Green);
        assert_eq!(buf[(2, 1)].symbol(), "▄");
        assert_eq!(buf[(2, 1)].fg, BEIGE_LIGHT);
        assert_eq!(buf[(2, area.bottom() - 1)].symbol(), "▀");
        assert_eq!(buf[(2, area.bottom() - 1)].fg, BEIGE_DARK);
        assert_eq!(buf[(2, 2)].symbol(), "▐");
        assert_eq!(buf[(area.right() - 1, 2)].symbol(), "▌");
        assert_eq!(buf[(10, 10)].bg, BEIGE, "the inside is filled");
        let row: String = (area.x..area.right())
            .map(|x| buf[(x, 2)].symbol().to_string())
            .collect();
        assert!(row.contains("SECURLOK"), "{row}");
        assert_eq!(buf[(area.right() - 3, 2)].symbol(), "●");
        assert_eq!(buf[(area.right() - 3, 2)].fg, LED_GREEN);
        bezel(&mut buf, area, "SECURLOK", Led::Red);
        assert_eq!(buf[(area.right() - 3, 2)].fg, LED_RED);
    }

    #[test]
    fn the_lcd_is_phosphor_on_dark_and_a_keycap_is_five_by_three() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        lcd(
            &mut buf,
            Rect::new(4, 3, 21, 4),
            &[("[4][4][_][_]".to_string(), Style::default().fg(PHOSPHOR))],
        );
        assert_eq!(buf[(4, 3)].symbol(), "┌");
        assert_eq!(buf[(6, 4)].symbol(), "[");
        assert_eq!(buf[(6, 4)].fg, PHOSPHOR);
        assert_eq!(buf[(6, 4)].bg, LCD_BG);
        keycap(&mut buf, 10, 10, '7', Style::default().fg(INK_WORN));
        assert_eq!(buf[(10, 10)].symbol(), "┌");
        assert_eq!(buf[(14, 12)].symbol(), "┘");
        assert_eq!(buf[(12, 11)].symbol(), "7");
        assert_eq!(buf[(12, 11)].fg, INK_WORN);
    }
}
