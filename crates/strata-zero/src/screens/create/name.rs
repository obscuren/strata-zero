//! Step 5: the handle, typed straight onto the sheet you have built.

use super::{Panes, summary, wrap};
use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, MAGENTA_RGB, TEXT};
use crate::ui::panel::boxed;
use crossterm::event::KeyCode;
use game::Build;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

const MARKS: &str = " -_.'";

pub(super) fn key(st: &mut AppState, code: KeyCode) {
    let max = st.content.creation.name_max;
    let Some(b) = st.build.as_mut() else { return };
    match code {
        KeyCode::Backspace => {
            b.handle.pop();
        }
        KeyCode::Char(c)
            if (c.is_alphanumeric() || MARKS.contains(c)) && b.handle.chars().count() < max =>
        {
            b.handle.push(c);
        }
        _ => {}
    }
}

pub(super) fn draw(st: &AppState, b: &Build, panes: &Panes, tick: u64, buf: &mut Buffer) {
    let left = panes.left;
    buf.set_string(left.x, left.y + 1, "handle", Style::default().fg(DIM));
    let y = left.y + 3;
    buf.set_string(left.x, y, "▸ ", Style::default().fg(CYAN));
    let shown = if b.handle.is_empty() {
        buf.set_string(left.x + 4, y, "type a name", Style::default().fg(DIM));
        0
    } else {
        buf.set_string(
            left.x + 2,
            y,
            &b.handle,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        );
        b.handle.chars().count() as u16
    };
    buf.set_string(
        left.x + 2 + shown,
        y,
        "▮",
        Style::default().fg(MAGENTA_RGB.scale(0.5 + 0.5 * pulse(tick, 20)).to_color()),
    );
    let rules = format!(
        "letters, digits, space and - _ . ' · {} at most",
        st.content.creation.name_max
    );
    for (i, l) in wrap(&rules, left.width.saturating_sub(2) as usize)
        .iter()
        .enumerate()
    {
        buf.set_string(left.x, y + 2 + i as u16, l, Style::default().fg(DIM));
    }

    // The sheet so far, with the name going on top of it.
    let block = boxed("THE SHEET");
    let inner = block.inner(panes.right);
    block.render(panes.right, buf);
    let name = b.handle.trim().to_uppercase();
    buf.set_string(
        inner.x + 2,
        inner.y + 1,
        if name.is_empty() {
            "no name yet"
        } else {
            &name
        },
        if name.is_empty() {
            Style::default().fg(DIM)
        } else {
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
        },
    );
    summary::sheet(
        st,
        b,
        Rect::new(
            inner.x + 2,
            inner.y + 3,
            inner.width.saturating_sub(4),
            inner.height.saturating_sub(4),
        ),
        buf,
    );
}
