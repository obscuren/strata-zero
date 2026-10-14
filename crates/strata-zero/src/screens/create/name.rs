//! Step 1: the handle, typed straight onto the sheet.

use super::{Panes, wrap};
use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, MAGENTA_RGB, TEXT};
use crate::ui::logo::{self, Logo};
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

pub(super) fn draw(
    st: &AppState,
    b: &Build,
    panes: &Panes,
    tall: bool,
    tick: u64,
    buf: &mut Buffer,
) {
    let left = panes.left;
    buf.set_string(left.x, left.y + 1, "handle", Style::default().fg(DIM));
    let y = left.y + 3;
    buf.set_string(left.x, y, "▸ ", Style::default().fg(CYAN));
    let shown = if b.handle.is_empty() {
        buf.set_string(left.x + 2, y, "type a name", Style::default().fg(DIM));
        11
    } else {
        buf.set_string(
            left.x + 2,
            y,
            &b.handle,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        );
        b.handle.chars().count() as u16
    };
    let cursor_x = left.x + 2 + if b.handle.is_empty() { 0 } else { shown };
    buf.set_string(
        cursor_x,
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

    // The sheet: the handle set large, so the limit is something you can see.
    let block = boxed("THE SHEET");
    let inner = block.inner(panes.right);
    block.render(panes.right, buf);
    let name = b.handle.trim();
    if name.is_empty() {
        buf.set_string(
            inner.x + 2,
            inner.y + 1,
            "nothing on it yet",
            Style::default().fg(DIM),
        );
        return;
    }
    let upper = name.to_uppercase();
    let w = logo::width(&upper);
    if tall && w + 4 <= inner.width {
        Logo {
            text: &upper,
            tick,
            accent: CYAN_RGB,
        }
        .render(Rect::new(inner.x + 2, inner.y + 1, w, logo::ROWS), buf);
    } else {
        buf.set_string(
            inner.x + 2,
            inner.y + 1,
            &upper,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        );
    }
}
