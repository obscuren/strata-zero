//! Step 5: the category board, bought with SP.

use super::{Create, FLASH_TICKS, Panes, wrap};
use crate::app::AppState;
use crate::ui::board::{BoardRow, BoardState, RowKind, board_rows};
use crate::ui::chrome::{BAR_BG, CYAN, CYAN_RGB, DIM, plain};
use crate::ui::meter::meter;
use crate::ui::panel::boxed;
use crossterm::event::KeyCode;
use game::{Build, CategoryId, Step};
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

fn rows(c: &Create, st: &AppState, b: &Build) -> Vec<BoardRow> {
    board_rows(
        &st.content,
        &BoardState {
            owned_cats: &b.categories,
            owned_skills: &b.skills,
            sp_left: b.sp_left(&st.content),
            open: &c.open,
        },
        st.tick,
        c.flash,
    )
}

/// The category a row belongs to: itself, or the one holding the skill.
fn category_of(st: &AppState, kind: &RowKind) -> Option<CategoryId> {
    match kind {
        RowKind::Category(id) => Some(id.clone()),
        RowKind::Skill(sid) => st.content.category_of(sid).map(|c| c.id.clone()),
        RowKind::Note => None,
    }
}

/// The nearest selectable row to `want`, searching forward then back.
fn clamp(rows: &[BoardRow], want: usize) -> usize {
    (want..rows.len())
        .chain((0..want).rev())
        .find(|&i| rows[i].selectable)
        .unwrap_or(0)
}

pub(super) fn key(c: &mut Create, st: &mut AppState, code: KeyCode) {
    let i = Step::Skills.index();
    let Some(b) = st.build.as_ref() else { return };
    let list = rows(c, st, b);
    if list.is_empty() {
        return;
    }
    let cur = clamp(&list, c.cursor[i]);
    let kind = list[cur].kind.clone();
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            let n = list.len();
            let mut j = cur;
            for _ in 0..n {
                j = (j + n - 1) % n;
                if list[j].selectable {
                    break;
                }
            }
            c.cursor[i] = j;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let n = list.len();
            let mut j = cur;
            for _ in 0..n {
                j = (j + 1) % n;
                if list[j].selectable {
                    break;
                }
            }
            c.cursor[i] = j;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(id) = category_of(st, &kind) {
                c.open.insert(id);
                c.cursor[i] = cur;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(id) = category_of(st, &kind) {
                c.open.remove(&id);
                // A folded skill's cursor lands on its category.
                let after = rows(c, st, b);
                c.cursor[i] = after
                    .iter()
                    .position(|r| matches!(&r.kind, RowKind::Category(cid) if *cid == id))
                    .unwrap_or(0);
            }
        }
        KeyCode::Char(' ') => {
            let content = &st.content;
            let Some(b) = st.build.as_mut() else { return };
            let result = match &kind {
                RowKind::Category(id) => b.toggle_category(content, id).map(|owned| {
                    if owned {
                        c.open.insert(id.clone());
                    }
                }),
                RowKind::Skill(id) => b.toggle_skill(content, id).map(|_| ()),
                RowKind::Note => return,
            };
            match result {
                Ok(()) => c.flash = Some((cur, st.tick + FLASH_TICKS)),
                Err(e) => c.say(e.to_string(), st.tick),
            }
            c.cursor[i] = cur;
        }
        _ => {}
    }
}

pub(super) fn draw(c: &Create, st: &AppState, b: &Build, panes: &Panes, buf: &mut Buffer) {
    let content = &st.content;
    let left = panes.left;
    let pool = meter("SP", b.sp_left(content), b.sp_total(content), CYAN_RGB);
    buf.set_line(left.x, left.y + 1, &pool, left.width);
    let list = rows(c, st, b);
    let cur = clamp(&list, c.cursor[Step::Skills.index()]);
    let top = left.y + 3;
    let visible = left.bottom().saturating_sub(top) as usize;
    let first = cur
        .saturating_sub(visible / 2)
        .min(list.len().saturating_sub(visible));
    for (k, row) in list.iter().enumerate().skip(first).take(visible) {
        let y = top + (k - first) as u16;
        let selected = k == cur;
        buf.set_string(
            left.x,
            y,
            if selected { "▸ " } else { "  " },
            Style::default().fg(CYAN),
        );
        let mut x = left.x + 2;
        for span in &row.line.spans {
            let mut style = span.style;
            if selected {
                style = style.bg(BAR_BG).add_modifier(Modifier::BOLD);
            }
            buf.set_string(x, y, span.content.as_ref(), style);
            x += span.content.width() as u16;
        }
    }

    // The dossier for the row under the cursor.
    let Some(row) = list.get(cur) else { return };
    let (title, lines) = match &row.kind {
        RowKind::Category(id) => {
            let Some(cat) = content.category(id) else {
                return;
            };
            let mut lines: Vec<Line<'static>> = vec![plain(cat.line.clone())];
            lines.push(Line::from(Span::styled(
                if b.categories.contains(id) {
                    "yours".to_string()
                } else {
                    format!("{} SP to unlock", cat.cost)
                },
                Style::default().fg(DIM),
            )));
            lines.push(plain(""));
            for sk in &cat.skills {
                let name = content
                    .skill(&sk.id)
                    .map(|k| k.name.clone())
                    .unwrap_or_else(|| sk.id.to_string());
                let have = b.skills.contains(&sk.id);
                let cost = if sk.cost == 0 {
                    "free".to_string()
                } else {
                    format!("{} SP", sk.cost)
                };
                lines.push(Line::from(Span::styled(
                    format!("{} {name} · {cost}", if have { "◆" } else { "◇" }),
                    Style::default().fg(if have { CYAN } else { DIM }),
                )));
            }
            for a in cat.grants.iter().filter_map(|id| content.ability(id)) {
                lines.push(plain(""));
                lines.push(Line::from(Span::styled(
                    format!("◈ {}", a.name),
                    Style::default().fg(CYAN),
                )));
                lines.extend(
                    wrap(&a.desc, panes.right.width.saturating_sub(6) as usize)
                        .into_iter()
                        .map(plain),
                );
            }
            if !cat.unlocks.is_empty() {
                lines.push(plain(""));
                lines.push(Line::from(Span::styled(
                    "▣ unlocks the workbench",
                    Style::default().fg(CYAN),
                )));
            }
            (cat.name.clone(), lines)
        }
        RowKind::Skill(id) => {
            let Some(skill) = content.skill(id) else {
                return;
            };
            let mut lines: Vec<Line<'static>> =
                wrap(&skill.desc, panes.right.width.saturating_sub(6) as usize)
                    .into_iter()
                    .map(plain)
                    .collect();
            lines.push(plain(""));
            let cat = content
                .category_of(id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let cost = match content.skill_cost(id) {
                Some(0) => "free with the category".to_string(),
                Some(n) => format!("{n} SP"),
                None => String::new(),
            };
            lines.push(Line::from(Span::styled(
                format!("{cat} · {} · {cost}", skill.stat.label()),
                Style::default().fg(DIM),
            )));
            (skill.name.to_uppercase(), lines)
        }
        RowKind::Note => return,
    };
    let block = boxed(&title);
    let inner = block.inner(panes.right);
    block.render(panes.right, buf);
    for (y, l) in (inner.y + 1..inner.bottom()).zip(lines) {
        buf.set_line(inner.x + 2, y, &l, inner.width.saturating_sub(3));
    }
}
