//! Step 6: the sheet, and JACK IN. The sheet itself also sits beside the name.

use super::stat_colour;
use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, MAGENTA, TEXT};
use game::{Build, BuildError, Stat};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The two-column sheet: stats and what you know on the left, where you were made,
/// what you did and your kit on the right.
pub(super) fn sheet(st: &AppState, b: &Build, area: Rect, buf: &mut Buffer) {
    let c = &st.content;
    let dim = Style::default().fg(DIM);
    let text = Style::default().fg(TEXT);
    let cyan = Style::default().fg(CYAN);
    let col = area.width / 2;
    let stop = area.bottom();

    let lx = area.x;
    let mut y = area.y;
    buf.set_string(lx, y, "STATS", dim);
    y += 1;
    for s in Stat::ALL {
        if y >= stop {
            return;
        }
        let v = b.stats.get(s);
        let bar = format!(
            "{}{}",
            "▰".repeat(v as usize),
            "▱".repeat(10usize.saturating_sub(v as usize))
        );
        let row = Line::from(vec![
            Span::styled(format!("{:<8}{v:>2}  ", s.name()), dim),
            Span::styled(bar, Style::default().fg(stat_colour(s))),
        ]);
        buf.set_line(lx, y, &row, col);
        y += 1;
    }
    y += 1;
    if y < stop {
        buf.set_string(lx, y, "SKILLS", dim);
        y += 1;
    }
    let mut any = false;
    for cat in c
        .categories
        .iter()
        .filter(|cat| b.categories.contains(&cat.id))
    {
        any = true;
        if y >= stop {
            break;
        }
        buf.set_string(lx, y, format!("◆ {}", cat.name), cyan);
        y += 1;
        let owned: Vec<String> = cat
            .skills
            .iter()
            .filter(|s| b.skills.contains(&s.id))
            .filter_map(|s| c.skill(&s.id).map(|k| k.name.clone()))
            .collect();
        if !owned.is_empty() && y < stop {
            buf.set_string(lx + 2, y, owned.join(" · "), dim);
            y += 1;
        }
        for a in cat.grants.iter().filter_map(|id| c.ability(id)) {
            if y >= stop {
                break;
            }
            buf.set_string(lx + 2, y, format!("◈ {}", a.name), cyan);
            y += 1;
        }
    }
    if !any && y < stop {
        buf.set_string(lx, y, "nothing yet", dim);
    }

    let rx = area.x + col;
    let mut y = area.y;
    let room = area.right().saturating_sub(rx + 12) as usize;
    if let Some(o) = b.origin.as_ref().and_then(|id| c.origin(id))
        && y + 1 < stop
    {
        buf.set_string(rx, y, "ORIGIN", dim);
        buf.set_string(
            rx + 12,
            y,
            &o.name,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        );
        buf.set_stringn(rx + 12, y + 1, &o.line, room, dim);
        y += 3;
    }
    if let Some(p) = b.profession.as_ref().and_then(|id| c.profession(id))
        && y + 1 < stop
    {
        buf.set_string(rx, y, "PROFESSION", dim);
        buf.set_string(
            rx + 12,
            y,
            &p.name,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        );
        buf.set_stringn(rx + 12, y + 1, &p.line, room, dim);
        y += 3;
        if y < stop {
            buf.set_string(rx, y, "KIT", dim);
            y += 1;
        }
        let mut kit: Vec<(game::ItemId, u32)> = p.kit.clone();
        kit.extend(b.kit.iter().cloned());
        for (id, n) in kit {
            if y >= stop {
                break;
            }
            let name = c
                .item(&id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| id.to_string());
            let row = if n > 1 { format!("{name} ×{n}") } else { name };
            buf.set_string(rx + 2, y, row, text);
            y += 1;
        }
        if y < stop {
            buf.set_string(rx + 2, y, format!("¤ {}", p.credits), dim);
        }
    }
}

pub(super) fn draw(st: &AppState, b: &Build, body: Rect, tick: u64, buf: &mut Buffer) {
    let c = &st.content;
    sheet(
        st,
        b,
        Rect::new(body.x, body.y, body.width, body.height.saturating_sub(2)),
        buf,
    );
    // Unspent or overspent, in magenta, and the confirm.
    let (pl, sl) = (b.points_left(c), b.sp_left(c));
    let fy = body.bottom().saturating_sub(1);
    if pl < 0 || sl < 0 {
        let mut parts = Vec::new();
        if pl < 0 {
            parts.push(BuildError::OverPoints((-pl) as u32).to_string());
        }
        if sl < 0 {
            parts.push(BuildError::OverSp((-sl) as u32).to_string());
        }
        buf.set_string(
            body.x,
            fy,
            parts.join(" · "),
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        );
    } else if pl > 0 || sl > 0 {
        buf.set_string(
            body.x,
            fy,
            format!("unspent · {pl} points · {sl} SP"),
            Style::default().fg(MAGENTA),
        );
    }
    let confirm = "[enter] JACK IN";
    let cx = body.x + body.width.saturating_sub(confirm.len() as u16) / 2;
    buf.set_string(
        cx,
        fy,
        confirm,
        Style::default()
            .fg(CYAN_RGB.scale(0.6 + 0.4 * pulse(tick, 24)).to_color())
            .add_modifier(Modifier::BOLD),
    );
}
