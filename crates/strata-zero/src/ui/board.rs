//! The skill category board: categories that fold and unfold over the skills they
//! hold, drawn the same way in the wizard and in the deck.

use crate::ui::chrome::{CYAN, DIM, TEXT};
use game::{CategoryId, Content, SkillId, Unlock};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    Category(CategoryId),
    Skill(SkillId),
    /// What a category grants or unlocks; never selectable.
    Note,
}

pub struct BoardRow {
    pub line: Line<'static>,
    pub kind: RowKind,
    pub selectable: bool,
}

/// What the board shows: what is owned, what can still be bought, what is unfolded.
pub struct BoardState<'a> {
    pub owned_cats: &'a BTreeSet<CategoryId>,
    pub owned_skills: &'a BTreeSet<SkillId>,
    pub sp_left: i64,
    pub open: &'a BTreeSet<CategoryId>,
}

/// One row per category; an open category is followed by its skills with connectors
/// and a note row for what it grants or unlocks. Owned reads `◆` in cyan, affordable
/// `◇` in text, anything out of reach `◇` dim. A row in `flash` draws inverted until
/// its tick.
pub fn board_rows(
    c: &Content,
    s: &BoardState,
    tick: u64,
    flash: Option<(usize, u64)>,
) -> Vec<BoardRow> {
    let mut rows = Vec::new();
    for cat in &c.categories {
        let owned = s.owned_cats.contains(&cat.id);
        let affordable = !owned && s.sp_left >= cat.cost as i64;
        let (glyph, fg) = if owned {
            ("◆", CYAN)
        } else if affordable {
            ("◇", TEXT)
        } else {
            ("◇", DIM)
        };
        let cost = if owned {
            "owned".to_string()
        } else if s.sp_left < cat.cost as i64 {
            "no SP".to_string()
        } else {
            format!("{} SP", cat.cost)
        };
        rows.push(BoardRow {
            line: Line::from(vec![
                Span::styled(format!("{glyph} "), Style::default().fg(fg)),
                Span::styled(
                    format!("{:<14}", cat.name),
                    Style::default().fg(fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(cost, Style::default().fg(DIM)),
            ]),
            kind: RowKind::Category(cat.id.clone()),
            selectable: true,
        });
        if !s.open.contains(&cat.id) {
            continue;
        }
        let n = cat.skills.len();
        for (i, sk) in cat.skills.iter().enumerate() {
            let name = c
                .skill(&sk.id)
                .map(|k| k.name.clone())
                .unwrap_or_else(|| sk.id.to_string());
            let have = s.owned_skills.contains(&sk.id);
            let can = owned && !have && s.sp_left >= sk.cost as i64;
            let (g, fg) = if have {
                ("◆", CYAN)
            } else if can {
                ("◇", TEXT)
            } else {
                ("◇", DIM)
            };
            let joint = if i + 1 == n { "└─" } else { "├─" };
            let cost = if sk.cost == 0 {
                "free".to_string()
            } else if owned && !have && s.sp_left < sk.cost as i64 {
                "no SP".to_string()
            } else {
                format!("{} SP", sk.cost)
            };
            rows.push(BoardRow {
                line: Line::from(vec![
                    Span::styled(format!("  {joint} "), Style::default().fg(DIM)),
                    Span::styled(format!("{g} "), Style::default().fg(fg)),
                    Span::styled(format!("{name:<14}"), Style::default().fg(fg)),
                    Span::styled(cost, Style::default().fg(DIM)),
                ]),
                kind: RowKind::Skill(sk.id.clone()),
                selectable: true,
            });
        }
        for id in &cat.grants {
            let name = c
                .ability(id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| id.to_string());
            rows.push(note(format!("     ◈ {name}"), owned));
        }
        for u in &cat.unlocks {
            match u {
                Unlock::Workbench => rows.push(note("     ▣ unlocks the workbench".into(), owned)),
            }
        }
    }
    if let Some((i, until)) = flash
        && tick < until
        && let Some(row) = rows.get_mut(i)
    {
        for span in &mut row.line.spans {
            span.style = span.style.add_modifier(Modifier::REVERSED);
        }
    }
    rows
}

fn note(text: String, lit: bool) -> BoardRow {
    BoardRow {
        line: Line::from(Span::styled(
            text,
            Style::default().fg(if lit { CYAN } else { DIM }),
        )),
        kind: RowKind::Note,
        selectable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome::DIM;
    use game::{Character, Content, TemplateId};
    use ratatui::style::Modifier;

    #[test]
    fn board_rows_fold_unfold_and_mark_ownership() {
        let c = Content::load().unwrap();
        let ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        let none: BTreeSet<CategoryId> = BTreeSet::new();
        let no_skills: BTreeSet<SkillId> = BTreeSet::new();
        let state = BoardState {
            owned_cats: &none,
            owned_skills: &no_skills,
            sp_left: 6,
            open: &none,
        };
        let rows = board_rows(&c, &state, 0, None);
        assert_eq!(rows.len(), 8);
        assert!(rows.iter().all(|r| r.line.to_string().starts_with("◇ ")));
        let first = rows[0].line.to_string();
        assert!(
            first.contains("NETRUNNER") && first.contains("2 SP"),
            "{first}"
        );
        assert!(rows.iter().all(|r| r.selectable));

        let open: BTreeSet<CategoryId> = ch.categories.clone();
        let state = BoardState {
            owned_cats: &ch.categories,
            owned_skills: &ch.skills,
            sp_left: 0,
            open: &open,
        };
        let rows = board_rows(&c, &state, 0, None);
        let texts: Vec<String> = rows.iter().map(|r| r.line.to_string()).collect();
        assert!(
            texts[0].starts_with("◆ NETRUNNER") && texts[0].contains("owned"),
            "{texts:?}"
        );
        assert!(
            texts[1].contains("├─ ◆ Hacking") && texts[1].contains("free"),
            "{texts:?}"
        );
        assert!(
            texts[2].contains("├─ ◆ ICE breaker") && texts[2].contains("1 SP"),
            "{texts:?}"
        );
        assert!(
            texts[3].contains("└─ ◇ Vault run") && texts[3].contains("no SP"),
            "a skill you could buy reads no SP when the pool is dry: {texts:?}"
        );
        assert!(texts[4].contains("◈ Signal ghost"), "{texts:?}");
        assert!(matches!(rows[4].kind, RowKind::Note) && !rows[4].selectable);
        assert!(
            texts.iter().any(|t| t.contains("▣ unlocks the workbench")),
            "{texts:?}"
        );
        let iron = rows
            .iter()
            .find(|r| matches!(&r.kind, RowKind::Category(id) if id.0 == "iron"))
            .unwrap();
        assert_eq!(
            iron.line.spans[0].style.fg,
            Some(DIM),
            "no SP: unowned reads dim"
        );
        assert!(iron.line.to_string().contains("no SP"), "{}", iron.line);
        let mut open_iron = open.clone();
        open_iron.insert(CategoryId::new("iron"));
        let state = BoardState {
            owned_cats: &ch.categories,
            owned_skills: &ch.skills,
            sp_left: 0,
            open: &open_iron,
        };
        let rows = board_rows(&c, &state, 0, None);
        let handguns = rows
            .iter()
            .find(|r| matches!(&r.kind, RowKind::Skill(id) if id.0 == "handguns"))
            .unwrap();
        assert!(
            handguns.line.to_string().contains("free"),
            "a locked skill keeps its price: {}",
            handguns.line
        );
        let flashed = board_rows(&c, &state, 10, Some((0, 20)));
        assert!(
            flashed[0]
                .line
                .spans
                .iter()
                .all(|s| s.style.add_modifier.contains(Modifier::REVERSED)),
            "a flashed row is inverted"
        );
        let cold = board_rows(&c, &state, 30, Some((0, 20)));
        assert!(
            !cold[0].line.spans[0]
                .style
                .add_modifier
                .contains(Modifier::REVERSED),
            "the flash ends"
        );
    }
}
