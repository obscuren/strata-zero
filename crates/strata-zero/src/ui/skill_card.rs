//! A skill's or a category's card from the board: its art at the top, then its text. The
//! deck reads what the character holds; the creation wizard reads what the build does.

use crate::screens::create::attribute_colour;
use crate::ui::chrome::{CYAN, DIM, FAINT, TEXT, plain, put_paragraph, reveal, warn};
use crate::ui::skill_art;
use game::{Attribute, CategoryId, Content, SkillArt, SkillId, Unlock};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::BTreeSet;

/// What a card reads as yours.
#[derive(Clone, Copy)]
pub struct Held<'a> {
    pub categories: &'a BTreeSet<CategoryId>,
    pub skills: &'a BTreeSet<SkillId>,
}

/// The board row a card is for.
#[derive(Clone, Copy)]
pub enum Card<'a> {
    Skill(&'a SkillId),
    Category(&'a CategoryId),
}

/// Draws a card into `area`: its art centred at the top, scanned in over the first half of
/// the reveal `p`, then its text typed out by `p`. When the text is too tall for the rows
/// left, its blank lines give way, top down.
pub fn draw(
    buf: &mut Buffer,
    area: Rect,
    content: &Content,
    held: Held,
    card: Card,
    p: f32,
    cursor_on: bool,
) {
    let mut text = area;
    if let Some((art, attribute, learned)) = art(content, held, card) {
        let scan = ((2.0 * p).min(1.0) * f32::from(skill_art::ROWS)).ceil() as u16;
        let x = area.x + area.width.saturating_sub(skill_art::W) / 2;
        skill_art::draw(buf, x, area.y, area, art, attribute, learned, scan);
        let drop = (skill_art::ROWS + 1).min(area.height);
        text.y += drop;
        text.height -= drop;
    }
    let lines = squeeze(lines(content, held, card), text.width, text.height);
    put_paragraph(buf, text, reveal(&lines, p, cursor_on));
}

/// The card's text, under its art. All but the last line, its description, are centred.
pub fn lines(content: &Content, held: Held, card: Card) -> Vec<Line<'static>> {
    match card {
        Card::Skill(id) => skill_lines(content, held, id),
        Card::Category(id) => category_lines(content, held, id),
    }
}

/// The art a card leads with, the attribute its sign is lit in and whether it is yours: a
/// skill's own, or a category's, lit in its first skill's attribute.
pub fn art<'a>(
    content: &'a Content,
    held: Held,
    card: Card,
) -> Option<(&'a SkillArt, Attribute, bool)> {
    match card {
        Card::Skill(id) => {
            let art = content.skill_art(id)?;
            Some((art, content.skill(id)?.attribute, held.skills.contains(id)))
        }
        Card::Category(id) => {
            let art = content.category_art(id)?;
            let first = content.category(id)?.skills.first()?;
            let attribute = content.skill(&first.id)?.attribute;
            Some((art, attribute, held.categories.contains(id)))
        }
    }
}

/// A skill's: whether it is yours, its price, the attributes and skills it asks for, its line.
fn skill_lines(content: &Content, held: Held, id: &SkillId) -> Vec<Line<'static>> {
    let Some(skill) = content.skill(id) else {
        return vec![warn(format!("unknown skill {id}"))];
    };
    let mut out = vec![if held.skills.contains(id) {
        Line::from(styled("[learned]", CYAN, true)).centered()
    } else {
        Line::from(styled("[unlearned]", DIM, false)).centered()
    }];
    if let Some(n) = content.skill_cost(id) {
        let cost = if n == 0 {
            "free".to_string()
        } else {
            format!("{n} SP")
        };
        out.push(framed(styled(cost, TEXT, false)));
    }
    for (attribute, n) in &skill.needs {
        let need = format!("{} {n}", attribute.name());
        out.push(framed(styled(need, attribute_colour(*attribute), true)));
    }
    out.push(plain(""));
    out.push(Line::from(styled("[REQUIRED SKILLS]", DIM, true)).centered());
    let after = if skill.after.is_empty() {
        styled("[none]", FAINT, false)
    } else {
        let names: Vec<&str> = skill
            .after
            .iter()
            .map(|a| content.skill(a).map_or(a.0.as_str(), |s| s.name.as_str()))
            .collect();
        styled(format!("[{}]", names.join(" · ")), TEXT, false)
    };
    out.push(Line::from(after).centered());
    out.push(plain(""));
    out.push(plain(skill.desc.clone()));
    out
}

/// A category's: whether it is yours, its price, its skills with their marks, what it
/// grants or unlocks, its line.
fn category_lines(content: &Content, held: Held, id: &CategoryId) -> Vec<Line<'static>> {
    let Some(cat) = content.category(id) else {
        return vec![warn(format!("unknown category {id}"))];
    };
    let owned = held.categories.contains(id);
    let mut out = vec![if owned {
        Line::from(styled("[unlocked]", CYAN, true)).centered()
    } else {
        Line::from(styled("[locked]", DIM, false)).centered()
    }];
    out.push(framed(styled(format!("{} SP", cat.cost), TEXT, false)));
    out.push(plain(""));
    out.push(Line::from(styled("[SKILLS]", DIM, true)).centered());
    let mut skills = Vec::new();
    for (i, sk) in cat.skills.iter().enumerate() {
        if i > 0 {
            skills.push(styled(" · ", FAINT, false));
        }
        let name = content
            .skill(&sk.id)
            .map_or_else(|| sk.id.to_string(), |k| k.name.clone());
        if held.skills.contains(&sk.id) {
            skills.push(styled("◆ ", CYAN, false));
            skills.push(styled(name, TEXT, false));
        } else {
            skills.push(styled(format!("◇ {name}"), DIM, false));
        }
    }
    out.push(Line::from(skills).centered());
    let lit = if owned { CYAN } else { DIM };
    for a in cat.grants.iter().filter_map(|a| content.ability(a)) {
        out.push(Line::from(styled(format!("◈ grants {}", a.name), lit, false)).centered());
        out.push(Line::from(styled(a.desc.clone(), FAINT, false)).centered());
    }
    for u in &cat.unlocks {
        let what = match u {
            Unlock::Workbench => "▣ unlocks the workbench",
        };
        out.push(Line::from(styled(what, lit, false)).centered());
    }
    out.push(plain(""));
    out.push(plain(cat.line.clone()));
    out
}

/// A card's text in one colour, bold or not.
fn styled(text: impl Into<String>, fg: Color, bold: bool) -> Span<'static> {
    let style = Style::default().fg(fg);
    let style = if bold {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    };
    Span::styled(text.into(), style)
}

/// A card's centred `:: value ::` line.
fn framed(inner: Span<'static>) -> Line<'static> {
    let rim = |s: &str| styled(s, FAINT, false);
    Line::from(vec![rim(":: "), inner, rim(" ::")]).centered()
}

/// Drops blank lines, top down, until `lines` fit `rows` at `width`.
fn squeeze(mut lines: Vec<Line<'static>>, width: u16, rows: u16) -> Vec<Line<'static>> {
    while height(&lines, width) > usize::from(rows) {
        let Some(i) = lines.iter().position(|l| l.width() == 0) else {
            break;
        };
        lines.remove(i);
    }
    lines
}

/// Rows `lines` take at `width`, wrapped greedily at spaces, a word longer than a row
/// spilling into the next.
fn height(lines: &[Line], width: u16) -> usize {
    let width = usize::from(width).max(1);
    lines
        .iter()
        .map(|l| {
            let (mut rows, mut used) = (1, 0usize);
            for word in l.to_string().split(' ').map(|w| w.chars().count()) {
                if word == 0 {
                    continue;
                }
                if used == 0 {
                    used = word;
                } else if used + 1 + word <= width {
                    used += 1 + word;
                } else {
                    rows += 1;
                    used = word;
                }
                while used > width {
                    rows += 1;
                    used -= width;
                }
            }
            rows
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_lines_give_way_when_the_text_runs_long() {
        let lines = vec![
            plain("one"),
            plain(""),
            plain("two"),
            plain(""),
            plain("three"),
        ];
        assert_eq!(
            squeeze(lines.clone(), 20, 5).len(),
            5,
            "it fits: nothing goes"
        );
        let squeezed = squeeze(lines.clone(), 20, 4);
        let texts: Vec<String> = squeezed.iter().map(|l| l.to_string()).collect();
        assert_eq!(
            texts,
            ["one", "two", "", "three"],
            "the first blank goes first"
        );
        assert_eq!(squeeze(lines, 20, 2).len(), 3, "only blank lines give way");
        assert_eq!(height(&[plain("aaaa bbbb cccc")], 9), 2);
    }
}
