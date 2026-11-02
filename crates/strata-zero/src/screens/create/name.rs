//! Step 5: the handle, typed straight onto the sheet you have built, and perma death
//! under it.

use super::{NameField, Panes, summary, wrap};
use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, FAINT, MAGENTA_RGB, RED, RED_TEXT, TEXT};
use crate::ui::panel::boxed;
use crossterm::event::KeyCode;
use game::Build;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

const MARKS: &str = " -_.'";

/// The tag THE SHEET wears beside the name while perma death is on.
const TAG: &str = "PERMA DEATH";

pub(super) fn key(st: &mut AppState, field: &mut NameField, code: KeyCode) {
    let max = st.content.creation.name_max;
    let Some(b) = st.build.as_mut() else { return };
    match (*field, code) {
        (_, KeyCode::Down) => *field = NameField::PermaDeath,
        (_, KeyCode::Up) => *field = NameField::Handle,
        (NameField::PermaDeath, KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')) => {
            b.perma_death = !b.perma_death;
        }
        (NameField::Handle, KeyCode::Backspace) => {
            b.handle.pop();
        }
        (NameField::Handle, KeyCode::Char(c))
            if (c.is_alphanumeric() || MARKS.contains(c)) && b.handle.chars().count() < max =>
        {
            b.handle.push(c);
        }
        _ => {}
    }
}

/// What perma death means, for the handle on the sheet.
fn meaning(b: &Build) -> String {
    if b.perma_death {
        let name = b.handle.trim().to_uppercase();
        let who = if name.is_empty() {
            "your runner"
        } else {
            &name
        };
        format!("One life. If {who} flatlines, the save goes too and the run is over.")
    } else {
        "A flatline sends you back to your last save.".to_string()
    }
}

pub(super) fn draw(
    st: &AppState,
    b: &Build,
    field: NameField,
    panes: &Panes,
    tick: u64,
    buf: &mut Buffer,
) {
    let left = panes.left;
    let width = left.width.saturating_sub(2) as usize;
    let marker = |buf: &mut Buffer, y: u16, on: bool| {
        if on {
            buf.set_string(left.x, y, "▸ ", Style::default().fg(CYAN));
        }
    };
    buf.set_string(left.x, left.y + 1, "handle", Style::default().fg(DIM));
    let y = left.y + 3;
    marker(buf, y, field == NameField::Handle);
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
    if field == NameField::Handle {
        buf.set_string(
            left.x + 2 + shown,
            y,
            "▮",
            Style::default().fg(MAGENTA_RGB.scale(0.5 + 0.5 * pulse(tick, 20)).to_color()),
        );
    }
    let rules = format!(
        "letters, digits, space and - _ . ' · {} at most",
        st.content.creation.name_max
    );
    let rules = wrap(&rules, width);
    for (i, l) in rules.iter().enumerate() {
        buf.set_string(left.x, y + 2 + i as u16, l, Style::default().fg(DIM));
    }

    // Perma death: ON · OFF, the pick bold in its colour, the other faint.
    let y = y + 2 + rules.len() as u16 + 2;
    buf.set_string(left.x, y, "perma death", Style::default().fg(DIM));
    let y = y + 2;
    marker(buf, y, field == NameField::PermaDeath);
    let pick = |on: bool, colour| {
        if on {
            Style::default().fg(colour).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FAINT)
        }
    };
    buf.set_string(left.x + 2, y, "ON", pick(b.perma_death, RED));
    buf.set_string(left.x + 4, y, " · ", Style::default().fg(FAINT));
    buf.set_string(left.x + 7, y, "OFF", pick(!b.perma_death, CYAN));
    let colour = if b.perma_death { RED_TEXT } else { DIM };
    for (i, l) in wrap(&meaning(b), width).iter().enumerate() {
        buf.set_string(left.x, y + 2 + i as u16, l, Style::default().fg(colour));
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
    if b.perma_death {
        let x = inner.right().saturating_sub(2 + TAG.len() as u16);
        buf.set_string(
            x,
            inner.y + 1,
            TAG,
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        );
    }
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
