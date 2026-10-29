//! Step 4: the five attributes, bought under affinity from one pool.

use super::{Create, FLASH_TICKS, Panes, attribute_colour, attribute_rgb, wrap};
use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, TEXT, plain};
use crate::ui::meter::meter;
use crate::ui::panel::boxed;
use crossterm::event::KeyCode;
use game::{Affinity, Attribute, Build, Step};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

fn word(shift: i32) -> &'static str {
    match shift {
        -1 => "cheap",
        1 => "dear",
        _ => "normal",
    }
}

pub(super) fn key(c: &mut Create, st: &mut AppState, code: KeyCode) {
    let i = Step::Attributes.index();
    match code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
            c.move_cursor(Attribute::ALL.len(), code);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let s = Attribute::ALL[c.cursor[i]];
            let content = &st.content;
            let Some(b) = st.build.as_mut() else { return };
            match b.buy_attribute(content, s) {
                Ok(()) => c.attribute_flash = Some((s, st.tick + FLASH_TICKS)),
                Err(e) => c.say(e.to_string(), st.tick),
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let s = Attribute::ALL[c.cursor[i]];
            let content = &st.content;
            let Some(b) = st.build.as_mut() else { return };
            if let Err(e) = b.refund_attribute(content, s) {
                c.say(e.to_string(), st.tick);
            }
        }
        _ => {}
    }
}

pub(super) fn draw(
    c: &Create,
    st: &AppState,
    b: &Build,
    panes: &Panes,
    tick: u64,
    buf: &mut Buffer,
) {
    let content = &st.content;
    let left = panes.left;
    let pool = meter(
        "POINTS",
        b.points_left(content),
        b.points_total(content),
        1,
        CYAN_RGB,
    );
    buf.set_line(left.x, left.y + 1, &pool, left.width);
    let cur = c.cursor[Step::Attributes.index()];
    for (k, s) in Attribute::ALL.iter().enumerate() {
        let y = left.y + 3 + k as u16 * 2;
        if y >= left.bottom() {
            break;
        }
        let v = b.attributes.get(*s) as usize;
        let selected = k == cur;
        let colour: Color = if selected {
            attribute_rgb(*s)
                .scale(0.7 + 0.3 * pulse(tick, 30))
                .to_color()
        } else {
            attribute_colour(*s)
        };
        let lit = c
            .attribute_flash
            .is_some_and(|(fs, until)| fs == *s && tick < until);
        let filled = v.min(10);
        let (steady, bright) = if lit && filled > 0 {
            (filled - 1, 1)
        } else {
            (filled, 0)
        };
        let cost = b.attribute_cost(content, *s);
        let tail = if v as u8 >= content.creation.attribute_cap {
            "maxed".to_string()
        } else {
            format!("next {cost} pt{}", if cost == 1 { "" } else { "s" })
        };
        let aff = b.affinity(content, *s);
        let mut spans = vec![
            Span::styled(
                if selected { "▸ " } else { "  " },
                Style::default().fg(CYAN),
            ),
            Span::styled(
                format!("{:<8}", s.name()),
                if selected {
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT)
                },
            ),
            Span::styled(format!("{v:>2}   "), Style::default().fg(TEXT)),
            Span::styled("▰".repeat(steady), Style::default().fg(colour)),
            Span::styled(
                "▰".repeat(bright),
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("▱".repeat(10 - filled), Style::default().fg(DIM)),
            Span::styled(format!("   {tail}"), Style::default().fg(DIM)),
        ];
        if aff != Affinity::Normal {
            spans.push(Span::styled(
                format!("  {}", aff.label()),
                Style::default().fg(if aff == Affinity::Cheap { CYAN } else { DIM }),
            ));
        }
        buf.set_line(left.x, y, &Line::from(spans), left.width);
    }

    // The dossier for the selected attribute.
    let s = Attribute::ALL[cur];
    let block = boxed(s.name());
    let inner = block.inner(panes.right);
    block.render(panes.right, buf);
    let x = inner.x + 2;
    let width = inner.width.saturating_sub(4) as usize;
    let mut lines: Vec<Line<'static>> = wrap(content.attribute_blurb(s), width)
        .into_iter()
        .map(plain)
        .collect();
    lines.push(plain(""));
    lines.push(Line::from(Span::styled("feeds", Style::default().fg(DIM))));
    for k in content.skills.iter().filter(|k| k.attribute == s) {
        let mark = if b.skills.contains(&k.id) {
            "◆"
        } else {
            "◇"
        };
        lines.push(Line::from(Span::styled(
            format!("{mark} {}", k.name),
            Style::default().fg(if b.skills.contains(&k.id) { CYAN } else { DIM }),
        )));
    }
    lines.push(plain(""));
    let origin = b
        .origin
        .as_ref()
        .and_then(|id| content.origin(id))
        .map(|o| o.name.to_lowercase())
        .unwrap_or_else(|| "—".into());
    let profession = b
        .profession
        .as_ref()
        .and_then(|id| content.profession(id))
        .map(|p| p.name.to_lowercase())
        .unwrap_or_else(|| "—".into());
    let row = |label: &str, name: String, w: &str| {
        Line::from(vec![
            Span::styled(format!("{label:<12}"), Style::default().fg(DIM)),
            Span::styled(format!("{name:<12}"), Style::default().fg(TEXT)),
            Span::styled(w.to_string(), Style::default().fg(DIM)),
        ])
    };
    lines.push(row("origin", origin, word(b.origin_shift(content, s))));
    lines.push(row(
        "profession",
        profession,
        word(b.profession_shift(content, s)),
    ));
    let cost = b.attribute_cost(content, s);
    lines.push(row(
        "together",
        b.affinity(content, s).label().to_string(),
        &format!("{cost} pt{} per point", if cost == 1 { "" } else { "s" }),
    ));
    for (y, l) in (inner.y + 1..inner.bottom()).zip(lines) {
        buf.set_line(x, y, &l, inner.width.saturating_sub(3));
    }
}
