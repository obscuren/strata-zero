//! Steps 2 and 3: a list to pick from, with a dossier that types itself out.

use super::{Panes, attribute_colour, wrap};

/// Rows each list entry takes: the name, up to two rows of tagline, a blank.
const STRIDE: u16 = 4;
use crate::anim::effects::typewriter;
use crate::anim::{Easing, Tween};
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, MAGENTA, TEXT, plain, reveal};
use crate::ui::panel::boxed;
use game::Attribute;
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// One thing to pick: what the list shows and what the dossier says about it.
pub(super) struct Entry {
    pub name: String,
    pub line: String,
    pub desc: String,
    pub emblem: Vec<String>,
    /// -1 cheap, 0 normal, +1 dear, per attribute.
    pub shifts: Vec<(Attribute, i32)>,
    pub kit: Vec<String>,
    pub credits: Option<u32>,
}

fn shifts(cheap: &[Attribute], dear: &[Attribute]) -> Vec<(Attribute, i32)> {
    Attribute::ALL
        .iter()
        .map(|&s| {
            let d = if cheap.contains(&s) {
                -1
            } else if dear.contains(&s) {
                1
            } else {
                0
            };
            (s, d)
        })
        .collect()
}

pub(super) fn origins(st: &AppState) -> Vec<Entry> {
    st.content
        .origins
        .iter()
        .map(|o| Entry {
            name: o.name.clone(),
            line: o.line.clone(),
            desc: o.desc.clone(),
            emblem: o.emblem.clone(),
            shifts: shifts(&o.cheap, &o.dear),
            kit: Vec::new(),
            credits: None,
        })
        .collect()
}

pub(super) fn professions(st: &AppState) -> Vec<Entry> {
    st.content
        .professions
        .iter()
        .map(|p| Entry {
            name: p.name.clone(),
            line: p.line.clone(),
            desc: p.desc.clone(),
            emblem: p.emblem.clone(),
            shifts: shifts(&p.cheap, &p.dear),
            kit: p
                .kit
                .iter()
                .map(|(id, n)| {
                    let name = st
                        .content
                        .item(id)
                        .map(|i| i.name.clone())
                        .unwrap_or_else(|| id.to_string());
                    if *n > 1 {
                        format!("{name} ×{n}")
                    } else {
                        name
                    }
                })
                .collect(),
            credits: Some(p.credits),
        })
        .collect()
}

/// The word and bar for a shift: cheap `▰▰▰▰▱`, normal `▰▰▰▱▱`, dear `▰▰▱▱▱`.
fn affinity(shift: i32) -> (&'static str, String) {
    let filled = (3 - shift).clamp(1, 5) as usize;
    let word = match shift {
        -1 => "cheap",
        1 => "dear",
        _ => "normal",
    };
    (
        word,
        format!("{}{}", "▰".repeat(filled), "▱".repeat(5 - filled)),
    )
}

pub(super) fn draw(
    entries: &[Entry],
    cursor: usize,
    opened: u64,
    changed: u64,
    tick: u64,
    panes: &Panes,
    buf: &mut Buffer,
) {
    let left = panes.left;
    for (i, e) in entries.iter().enumerate() {
        let y = left.y + 1 + i as u16 * STRIDE;
        if y + 1 >= left.bottom() {
            break;
        }
        let p = Tween::new(opened + i as u64 * 4, 8, Easing::OutCubic).progress(tick);
        let selected = i == cursor;
        let name = typewriter(&e.name, p).to_string();
        let line = typewriter(&e.line, p).to_string();
        // The name, then what it makes cheap and dear at a glance.
        let cheap: Vec<&str> = e
            .shifts
            .iter()
            .filter(|(_, d)| *d < 0)
            .map(|(s, _)| s.name())
            .collect();
        let dear: Vec<&str> = e
            .shifts
            .iter()
            .filter(|(_, d)| *d > 0)
            .map(|(s, _)| s.name())
            .collect();
        let row = Line::from(vec![
            Span::styled(
                if selected { "▸ " } else { "  " },
                Style::default().fg(CYAN),
            ),
            Span::styled(
                format!("{name:<14}"),
                if selected {
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT)
                },
            ),
            Span::styled(cheap.join(" "), Style::default().fg(CYAN)),
            Span::styled(" · ", Style::default().fg(DIM)),
            Span::styled(dear.join(" "), Style::default().fg(MAGENTA)),
        ]);
        buf.set_line(left.x, y, &row, left.width);
        let room = left.width.saturating_sub(2) as usize;
        for (k, part) in wrap(&line, room).into_iter().take(2).enumerate() {
            let tag = Line::from(Span::styled(part, Style::default().fg(DIM)));
            buf.set_line(left.x + 2, y + 1 + k as u16, &tag, room as u16);
        }
    }

    let Some(e) = entries.get(cursor) else { return };
    let block = boxed("DOSSIER");
    let inner = block.inner(panes.right);
    block.render(panes.right, buf);
    let x = inner.x + 2;
    let mut y = inner.y + 1;
    for (r, row) in e.emblem.iter().enumerate() {
        buf.set_string(x, y + r as u16, row, Style::default().fg(CYAN));
    }
    buf.set_string(
        x + 10,
        y + 1,
        &e.name,
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    );
    buf.set_stringn(
        x + 10,
        y + 2,
        &e.line,
        inner.right().saturating_sub(x + 11) as usize,
        Style::default().fg(DIM),
    );
    y += 4;
    let width = inner.width.saturating_sub(4) as usize;
    let lines: Vec<Line<'static>> = wrap(&e.desc, width).into_iter().map(plain).collect();
    let p = ((tick.saturating_sub(changed)) as f32 / 30.0).clamp(0.0, 1.0);
    for l in reveal(&lines, p, p < 1.0) {
        if y >= inner.bottom() {
            return;
        }
        buf.set_line(x, y, &l, inner.width.saturating_sub(3));
        y += 1;
    }
    y += 1;
    buf.set_string(x, y, "AFFINITY", Style::default().fg(DIM));
    y += 1;
    for (s, d) in &e.shifts {
        if y >= inner.bottom() {
            return;
        }
        let (word, bar) = affinity(*d);
        let row = Line::from(vec![
            Span::styled(format!("{:<8} ", s.name()), Style::default().fg(DIM)),
            Span::styled(bar, Style::default().fg(attribute_colour(*s))),
            Span::styled(format!("   {word}"), Style::default().fg(DIM)),
        ]);
        buf.set_line(x, y, &row, inner.width.saturating_sub(3));
        y += 1;
    }
    if !e.kit.is_empty() {
        y += 1;
        buf.set_string(x, y, "KIT", Style::default().fg(DIM));
        y += 1;
        for item in &e.kit {
            if y >= inner.bottom() {
                return;
            }
            buf.set_string(x, y, item, Style::default().fg(TEXT));
            y += 1;
        }
        if let Some(c) = e.credits
            && y < inner.bottom()
        {
            buf.set_string(x, y, format!("¤ {c}"), Style::default().fg(DIM));
        }
    }
}
