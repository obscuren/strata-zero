//! Step 6: the sheet, and JACK IN.

use super::stat_colour;
use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, MAGENTA, TEXT};
use crate::ui::logo::{self, Logo};
use game::{Build, Stat};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

pub(super) fn draw(st: &AppState, b: &Build, body: Rect, tall: bool, tick: u64, buf: &mut Buffer) {
    let c = &st.content;
    let mut y = body.y;
    let handle = b.handle.trim().to_uppercase();
    let hw = logo::width(&handle);
    if tall && !handle.is_empty() && hw <= body.width {
        Logo {
            text: &handle,
            tick,
            accent: CYAN_RGB,
        }
        .render(Rect::new(body.x, y, hw, logo::ROWS), buf);
        y += logo::ROWS + 1;
    } else {
        buf.set_string(
            body.x,
            y,
            if handle.is_empty() { "—" } else { &handle },
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        );
        y += 2;
    }
    let top = y;
    let col = body.width / 2;
    let dim = Style::default().fg(DIM);
    let text = Style::default().fg(TEXT);
    let cyan = Style::default().fg(CYAN);

    // Left: stats, then owned categories with their owned skills.
    let lx = body.x;
    buf.set_string(lx, y, "STATS", dim);
    y += 1;
    for s in Stat::ALL {
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
    buf.set_string(lx, y, "SKILLS", dim);
    y += 1;
    let mut any = false;
    for cat in c
        .categories
        .iter()
        .filter(|cat| b.categories.contains(&cat.id))
    {
        any = true;
        if y >= body.bottom().saturating_sub(2) {
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
        if !owned.is_empty() && y < body.bottom().saturating_sub(2) {
            buf.set_string(lx + 2, y, owned.join(" · "), dim);
            y += 1;
        }
    }
    if !any {
        buf.set_string(lx, y, "nothing yet", dim);
    }

    // Right: origin, profession, kit, credits.
    let rx = body.x + col;
    let mut y = top;
    if let Some(o) = b.origin.as_ref().and_then(|id| c.origin(id)) {
        buf.set_string(rx, y, "ORIGIN", dim);
        buf.set_string(
            rx + 12,
            y,
            &o.name,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        );
        buf.set_stringn(
            rx + 12,
            y + 1,
            &o.line,
            body.right().saturating_sub(rx + 12) as usize,
            dim,
        );
        y += 3;
    }
    if let Some(p) = b.profession.as_ref().and_then(|id| c.profession(id)) {
        buf.set_string(rx, y, "PROFESSION", dim);
        buf.set_string(
            rx + 12,
            y,
            &p.name,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        );
        buf.set_stringn(
            rx + 12,
            y + 1,
            &p.line,
            body.right().saturating_sub(rx + 12) as usize,
            dim,
        );
        y += 3;
        buf.set_string(rx, y, "KIT", dim);
        y += 1;
        let mut kit: Vec<(game::ItemId, u32)> = p.kit.clone();
        kit.extend(b.kit.iter().cloned());
        for (id, n) in kit {
            if y >= body.bottom().saturating_sub(2) {
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
        buf.set_string(rx + 2, y, format!("¤ {}", p.credits), dim);
    }

    // Unspent or overspent, in magenta, and the confirm.
    let (pl, sl) = (b.points_left(c), b.sp_left(c));
    let fy = body.bottom().saturating_sub(1);
    if pl < 0 || sl < 0 {
        let mut parts = Vec::new();
        if pl < 0 {
            parts.push(format!("over by {} points", -pl));
        }
        if sl < 0 {
            parts.push(format!("over by {} SP", -sl));
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
