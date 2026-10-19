//! The deck's chrome, shared by every panel drawn in its register: colours, the glass
//! wash, heavy rules, text on a solid backing and the typewriter reveal.

use crate::anim::effects::typewriter;
use crate::render::Rgb;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub(crate) const CYAN: Color = Color::Rgb(0, 229, 255);
pub(crate) const MAGENTA: Color = Color::Rgb(255, 0, 200);
pub(crate) const DIM: Color = Color::Rgb(160, 170, 185);
/// Brackets, rules and other furniture: darker than labels.
pub(crate) const FAINT: Color = Color::Rgb(120, 130, 145);
/// Darker than FAINT: what is there but empty or out of reach.
pub(crate) const FAINTER: Color = Color::Rgb(0x5c, 0x66, 0x73);
pub(crate) const TEXT: Color = Color::Rgb(205, 212, 222);
pub(crate) const CYAN_RGB: Rgb = Rgb(0, 229, 255);
pub(crate) const MAGENTA_RGB: Rgb = Rgb(255, 0, 200);
pub(crate) const PANEL_BG: Color = Color::Rgb(4, 6, 10);
pub(crate) const PANEL_BG_RGB: Rgb = Rgb(4, 6, 10);
/// How far the washed map is pulled toward the panel colour; the rest shows through.
pub(crate) const GLASS: f32 = 0.45;
/// What a terminal's faint attribute does to a colour, roughly; the glass bakes it in.
const FAINT_GHOST: f32 = 0.5;
pub(crate) const RULE: Color = Color::Rgb(40, 60, 75);
pub(crate) const BAR_BG: Color = Color::Rgb(0, 60, 70);

pub(crate) fn head(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into().to_uppercase(),
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    ))
}

pub(crate) fn plain(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(TEXT)))
}

pub(crate) fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

pub(crate) fn warn(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(MAGENTA)))
}

/// Heavy top and bottom rules, light sides.
pub(crate) fn frame(buf: &mut Buffer, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let style = Style::default().fg(CYAN);
    for x in area.x..area.right() {
        buf[(x, area.y)].set_char('━').set_style(style);
        buf[(x, area.bottom() - 1)].set_char('━').set_style(style);
    }
    for y in area.y..area.bottom() {
        buf[(area.x, y)].set_char('│').set_style(style);
        buf[(area.right() - 1, y)].set_char('│').set_style(style);
    }
    buf[(area.x, area.y)].set_char('┍').set_style(style);
    buf[(area.right() - 1, area.y)]
        .set_char('┑')
        .set_style(style);
    buf[(area.x, area.bottom() - 1)]
        .set_char('┕')
        .set_style(style);
    buf[(area.right() - 1, area.bottom() - 1)]
        .set_char('┙')
        .set_style(style);
}

/// Tints the panel area toward the panel colour, keeping the washed map as a ghost.
/// The ghost is colour only: a faint cell is darkened instead, and every modifier goes,
/// or text written over it later (`set_style` only adds modifiers) would inherit them.
pub(crate) fn glass(buf: &mut Buffer, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            let bg = match cell.bg {
                Color::Rgb(r, g, b) => Rgb(r, g, b).mix(PANEL_BG_RGB, GLASS).to_color(),
                _ => PANEL_BG,
            };
            let ghost = if cell.modifier.contains(Modifier::DIM) {
                0.6 * FAINT_GHOST
            } else {
                0.6
            };
            let fg = match cell.fg {
                Color::Rgb(r, g, b) => Rgb(r, g, b).scale(ghost).to_color(),
                other => other,
            };
            cell.set_bg(bg).set_fg(fg);
            cell.modifier = Modifier::empty();
        }
    }
}

/// Blanks the panel area onto the panel colour: whatever was beneath no longer shows.
pub(crate) fn solid(buf: &mut Buffer, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            cell.reset();
            cell.set_bg(PANEL_BG);
        }
    }
}

/// Writes `text` from `x`, stopping before `max_x`, on a solid backing unless the style
/// brings its own background.
pub(crate) fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, max_x: u16) {
    let style = if style.bg.is_none() {
        style.bg(PANEL_BG)
    } else {
        style
    };
    for (cx, ch) in (x..max_x).zip(text.chars()) {
        buf[(cx, y)].set_char(ch).set_style(style);
    }
}

/// Renders lines into `area`, backing only the cells that carry text.
pub(crate) fn put_paragraph(buf: &mut Buffer, area: Rect, lines: Vec<Line<'static>>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut tmp = Buffer::empty(area);
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(area, &mut tmp);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let src = &tmp[(x, y)];
            if src.symbol() != " " {
                let style = src.style().bg(PANEL_BG);
                buf[(x, y)].set_symbol(src.symbol()).set_style(style);
            }
        }
    }
}

/// Reveals `lines` progressively: `p` of all characters are shown, then a cursor.
pub(crate) fn reveal(lines: &[Line<'static>], p: f32, cursor_on: bool) -> Vec<Line<'static>> {
    if p >= 1.0 {
        return lines.to_vec();
    }
    let total: usize = lines.iter().map(|l| l.width()).sum();
    let mut budget = ((total as f32) * p.clamp(0.0, 1.0)).floor() as usize;
    let cursor = || Span::styled("▮", Style::default().fg(MAGENTA));
    let mut out = Vec::new();
    for line in lines {
        let w = line.width();
        if w <= budget {
            out.push(line.clone());
            budget -= w;
            continue;
        }
        let mut shown: Vec<Span<'static>> = Vec::new();
        let mut left = budget;
        for span in &line.spans {
            let text = span.content.to_string();
            let n = text.chars().count();
            if n <= left {
                shown.push(span.clone());
                left -= n;
            } else {
                let frac = if n == 0 { 1.0 } else { left as f32 / n as f32 };
                shown.push(Span::styled(
                    typewriter(&text, frac).to_string(),
                    span.style,
                ));
                break;
            }
        }
        if cursor_on {
            shown.push(cursor());
        }
        out.push(Line::from(shown));
        return out;
    }
    if cursor_on {
        out.push(Line::from(cursor()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted() -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
        for y in 0..3 {
            for x in 0..6 {
                buf[(x, y)]
                    .set_char('x')
                    .set_fg(Color::Rgb(200, 200, 200))
                    .set_bg(Color::Rgb(90, 10, 10));
            }
        }
        buf
    }

    #[test]
    fn solid_is_opaque() {
        let mut buf = painted();
        solid(&mut buf, Rect::new(1, 1, 4, 1));
        for x in 1..5 {
            assert_eq!(buf[(x, 1)].symbol(), " ");
            assert_eq!(buf[(x, 1)].bg, PANEL_BG);
        }
        assert_eq!(buf[(0, 1)].symbol(), "x");
        assert_eq!(buf[(1, 0)].symbol(), "x");
    }

    #[test]
    fn glass_ghosts_what_is_beneath() {
        let mut buf = painted();
        glass(&mut buf, Rect::new(1, 1, 4, 1));
        let cell = &buf[(2, 1)];
        assert_eq!(cell.symbol(), "x", "the glyph beneath stays as a ghost");
        assert_ne!(
            cell.bg,
            Color::Rgb(90, 10, 10),
            "bg is tinted toward the panel"
        );
        assert_ne!(cell.bg, PANEL_BG, "bg is not solid");
        let Color::Rgb(r, _, _) = cell.fg else {
            panic!()
        };
        assert!(r < 200, "fg is dimmed");
    }

    #[test]
    fn glass_bakes_a_faint_ghost_into_its_colour_and_text_over_it_is_clean() {
        let mut buf = painted();
        buf[(2, 1)].modifier.insert(Modifier::DIM | Modifier::BOLD);
        glass(&mut buf, Rect::new(1, 1, 4, 1));
        let Color::Rgb(faint, _, _) = buf[(2, 1)].fg else {
            panic!()
        };
        let Color::Rgb(plain, _, _) = buf[(3, 1)].fg else {
            panic!()
        };
        assert!(
            faint < plain,
            "a faint ghost stays fainter: {faint} vs {plain}"
        );
        assert_eq!(buf[(2, 1)].modifier, Modifier::empty());
        put(&mut buf, 1, 1, "abcd", Style::default().fg(TEXT), 5);
        for x in 1..5 {
            assert_eq!(buf[(x, 1)].modifier, Modifier::empty(), "x {x}");
        }
    }

    #[test]
    fn dim_reads_on_black_and_faint_is_darker() {
        let Color::Rgb(r, g, b) = DIM else { panic!() };
        assert!(r >= 150 && g >= 150 && b >= 150, "{DIM:?}");
        let Color::Rgb(fr, fg, fb) = FAINT else {
            panic!()
        };
        assert!(fr < r && fg < g && fb < b, "{FAINT:?}");
    }
}
