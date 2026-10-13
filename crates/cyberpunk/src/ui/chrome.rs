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
pub(crate) const DIM: Color = Color::Rgb(120, 130, 145);
pub(crate) const TEXT: Color = Color::Rgb(205, 212, 222);
pub(crate) const CYAN_RGB: Rgb = Rgb(0, 229, 255);
pub(crate) const MAGENTA_RGB: Rgb = Rgb(255, 0, 200);
pub(crate) const PANEL_BG: Color = Color::Rgb(4, 6, 10);
pub(crate) const PANEL_BG_RGB: Rgb = Rgb(4, 6, 10);
/// How far the washed map is pulled toward the panel colour; the rest shows through.
pub(crate) const GLASS: f32 = 0.45;
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
pub(crate) fn glass(buf: &mut Buffer, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            let bg = match cell.bg {
                Color::Rgb(r, g, b) => Rgb(r, g, b).mix(PANEL_BG_RGB, GLASS).to_color(),
                _ => PANEL_BG,
            };
            let fg = match cell.fg {
                Color::Rgb(r, g, b) => Rgb(r, g, b).scale(0.6).to_color(),
                other => other,
            };
            cell.set_bg(bg).set_fg(fg);
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
