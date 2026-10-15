//! The talent trees in the deck: five columns of tiers, a box per talent with its
//! rank meter, connectors down to what a talent needs, and a dossier for the one
//! under the cursor.

use crate::screens::create::stat_colour;
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINT, MAGENTA, PANEL_BG, RULE, TEXT, put, put_paragraph, reveal,
};
use game::{Character, Content, Talent, TalentId};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// A column's width at most: two boxes of twelve; a narrower pane shrinks it.
pub const COL_W: u16 = 27;
/// Rows the dossier takes: a rule and four lines.
pub const DOSSIER_ROWS: u16 = 5;
/// Ticks the dossier takes to type itself out.
const REVEAL_TICKS: u64 = 20;

/// Where the cursor sits: a tree, a tier from the bottom, a column along the tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeCursor {
    pub tree: usize,
    pub tier: usize,
    pub col: usize,
}

impl TreeCursor {
    /// Packs the cursor into one number for the deck's row memory.
    pub fn pack(self) -> usize {
        self.tree * 100 + self.tier * 10 + self.col
    }

    pub fn unpack(v: usize) -> TreeCursor {
        TreeCursor {
            tree: v / 100,
            tier: v / 10 % 10,
            col: v % 10,
        }
    }
}

/// The talent under a cursor.
pub fn talent_at(c: &Content, cur: TreeCursor) -> Option<&TalentId> {
    c.trees.get(cur.tree)?.tiers.get(cur.tier)?.get(cur.col)
}

/// The nearest cursor that sits on a talent.
fn clamp(c: &Content, mut cur: TreeCursor) -> TreeCursor {
    let Some(last) = c.trees.len().checked_sub(1) else {
        return TreeCursor::default();
    };
    cur.tree = cur.tree.min(last);
    let tree = &c.trees[cur.tree];
    let Some(last_tier) = tree.tiers.len().checked_sub(1) else {
        return TreeCursor {
            tree: cur.tree,
            tier: 0,
            col: 0,
        };
    };
    cur.tier = cur.tier.min(last_tier);
    cur.col = cur.col.min(tree.tiers[cur.tier].len().saturating_sub(1));
    cur
}

/// Moves along a tier and on into the next tree for `dx`, between tiers of the same
/// tree for `dy`, landing on the nearest column; both wrap.
pub fn step(c: &Content, cur: TreeCursor, dx: i32, dy: i32) -> TreeCursor {
    let cur = clamp(c, cur);
    if c.trees.is_empty() {
        return cur;
    }
    let n_trees = c.trees.len() as i32;
    let mut out = cur;
    if dx != 0 {
        let width = c.trees[cur.tree].tiers.get(cur.tier).map_or(0, |t| t.len()) as i32;
        let col = cur.col as i32 + dx;
        if col < 0 {
            out.tree = (cur.tree as i32 - 1).rem_euclid(n_trees) as usize;
            out.col = c.trees[out.tree]
                .tiers
                .get(cur.tier)
                .map_or(0, |t| t.len().saturating_sub(1));
        } else if col >= width {
            out.tree = (cur.tree as i32 + 1).rem_euclid(n_trees) as usize;
            out.col = 0;
        } else {
            out.col = col as usize;
        }
    }
    if dy != 0 {
        let tiers = c.trees[out.tree].tiers.len() as i32;
        if tiers > 0 {
            out.tier = (out.tier as i32 + dy).rem_euclid(tiers) as usize;
        }
    }
    clamp(c, out)
}

/// How a box is lit: plain, under the cursor, or flashing with a fresh rank.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoxLit {
    Plain,
    Cursor,
    Flash,
}

/// How a talent reads right now.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// A point would go in.
    Affordable,
    /// Every rank taken.
    Maxed,
    /// Out of reach for now.
    Out,
}

fn reach(c: &Content, ch: &Character, t: &Talent) -> Reach {
    if ch.can_learn(c, &t.id).is_ok() {
        Reach::Affordable
    } else if ch.talent_rank(&t.id) >= t.ranks {
        Reach::Maxed
    } else {
        Reach::Out
    }
}

fn words(c: &Content, t: &Talent, rank: usize) -> String {
    t.effects
        .get(rank)
        .map(|bs| {
            bs.iter()
                .map(|b| b.words(c))
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

/// Four lines on a talent: where it hangs and its rank; what the next rank brings,
/// or why it is out of reach; what is held; the description.
pub fn dossier(c: &Content, ch: &Character, id: &TalentId) -> Vec<Line<'static>> {
    let Some(t) = c.talent(id) else {
        return Vec::new();
    };
    let (tree_name, tier) = c
        .tree_of(id)
        .map(|(tree, tier, _)| (tree.name.clone(), tier + 1))
        .unwrap_or_default();
    let rank = ch.talent_rank(id);
    let dim = Style::default().fg(DIM);
    let text = Style::default().fg(TEXT);
    let head = format!(
        "{} · {tree_name} · tier {tier} · rank {rank}/{}",
        t.name.to_uppercase(),
        t.ranks
    );
    let mut lines = vec![Line::from(Span::styled(
        head,
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    ))];
    let next = match ch.can_learn(c, id) {
        Ok(()) => Line::from(vec![
            Span::styled("next: ", dim),
            Span::styled(words(c, t, rank as usize), text),
        ]),
        Err(_) if rank >= t.ranks => {
            Line::from(Span::styled("at its best", Style::default().fg(MAGENTA)))
        }
        Err(why) => Line::from(vec![
            Span::styled(format!("locked · {why}"), Style::default().fg(MAGENTA)),
            Span::styled(" · next: ", dim),
            Span::styled(words(c, t, rank as usize), text),
        ]),
    };
    lines.push(next);
    let held: Vec<String> = (0..rank as usize)
        .map(|r| words(c, t, r))
        .filter(|w| !w.is_empty())
        .collect();
    lines.push(Line::from(vec![
        Span::styled("now: ", dim),
        if held.is_empty() {
            Span::styled("nothing yet", dim)
        } else {
            Span::styled(held.join(" · "), text)
        },
    ]));
    lines.push(Line::from(Span::styled(t.desc.clone(), text)));
    lines
}

/// The trees drawn across a pane: a header with the points, five columns of tiers,
/// the dossier of the talent under the cursor at the bottom.
pub struct TalentsView<'a> {
    pub content: &'a Content,
    pub character: &'a Character,
    pub cursor: TreeCursor,
    pub tick: u64,
    /// Tick the cursor last moved: the dossier types itself out from there.
    pub changed: u64,
    /// A box lit in magenta until its tick: the rank just learned.
    pub flash: Option<(TalentId, u64)>,
}

impl TalentsView<'_> {
    pub fn draw(&self, area: Rect, buf: &mut Buffer) {
        let (c, ch) = (self.content, self.character);
        if area.height < DOSSIER_ROWS + 2 || area.width < 10 || c.trees.is_empty() {
            return;
        }
        let right = area.right();

        // Header: the points left, then each tree's count in its colour.
        let pts = ch.talent_points;
        let head = format!("TALENTS · {pts} point{}", if pts == 1 { "" } else { "s" });
        let mut x = area.x + 1;
        put(
            buf,
            x,
            area.y,
            &head,
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            right,
        );
        x += head.chars().count() as u16 + 4;
        for (i, tree) in c.trees.iter().enumerate() {
            if i > 0 {
                put(buf, x, area.y, " · ", Style::default().fg(DIM), right);
                x += 3;
            }
            let s = format!("{} {}", tree.name, ch.tree_points(c, tree.stat));
            put(
                buf,
                x,
                area.y,
                &s,
                Style::default().fg(stat_colour(tree.stat)),
                right,
            );
            x += s.chars().count() as u16;
        }

        // Dossier under a rule at the bottom.
        let rule_y = area.bottom() - DOSSIER_ROWS;
        for cx in area.x..right {
            buf[(cx, rule_y)].set_char('─').set_fg(RULE);
        }
        if let Some(id) = talent_at(c, self.cursor) {
            let lines = dossier(c, ch, id);
            let p = self.tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
            let shown = reveal(&lines, p, (self.tick / 4).is_multiple_of(2));
            put_paragraph(
                buf,
                Rect::new(
                    area.x + 1,
                    rule_y + 1,
                    area.width.saturating_sub(2),
                    DOSSIER_ROWS - 1,
                ),
                shown,
            );
        }

        // Columns: the tree's name and line, then tiers of boxes with a gap row between.
        let col_w = (area.width / c.trees.len() as u16).min(COL_W);
        if col_w < 7 {
            return;
        }
        // Two boxes a column with a cell between and a cell after.
        let box_w = (col_w - 3) / 2;
        let top = area.y + 1;
        let avail = rule_y.saturating_sub(top);
        let max_tiers = c.trees.iter().map(|t| t.tiers.len()).max().unwrap_or(0) as u16;
        let show_line = avail >= 2 + (3 * max_tiers).saturating_sub(1);
        let tier_rows = avail.saturating_sub(if show_line { 2 } else { 1 });
        let tiers_fit = ((tier_rows + 1) / 3) as usize;
        for (ti, tree) in c.trees.iter().enumerate() {
            let cx = area.x + ti as u16 * col_w;
            let colour = stat_colour(tree.stat);
            let mut y = top;
            put(
                buf,
                cx + 1,
                y,
                &tree.name,
                Style::default().fg(colour).add_modifier(Modifier::BOLD),
                cx + col_w,
            );
            y += 1;
            if show_line {
                put(
                    buf,
                    cx + 1,
                    y,
                    &tree.line,
                    Style::default().fg(FAINT),
                    cx + col_w,
                );
                y += 1;
            }
            for (tier_i, tier) in tree.tiers.iter().enumerate().take(tiers_fit) {
                if tier_i > 0 {
                    // The gap row: a closed tier's caption, else connectors down to
                    // what the tier's talents need.
                    let need = c.creation.tier_gate * tier_i as u32;
                    let have = ch.tree_points(c, tree.stat);
                    if have < need {
                        let more = need - have;
                        let cap = format!("── {more} more ──");
                        let w = cap.chars().count() as u16;
                        let x = cx + (col_w.saturating_sub(w)) / 2;
                        put(buf, x, y, &cap, Style::default().fg(FAINT), cx + col_w);
                    } else {
                        for (col, id) in tier.iter().enumerate() {
                            if c.talent(id).is_some_and(|t| t.requires.is_some()) {
                                let x = cx + 1 + col as u16 * (box_w + 1) + box_w / 2;
                                if x < cx + col_w {
                                    buf[(x, y)].set_char('│').set_fg(RULE);
                                }
                            }
                        }
                    }
                    y += 1;
                }
                for (col, id) in tier.iter().enumerate() {
                    let Some(t) = c.talent(id) else {
                        continue;
                    };
                    let x = cx + 1 + col as u16 * (box_w + 1);
                    if x + box_w > cx + col_w {
                        break;
                    }
                    let is_cursor = self.cursor
                        == TreeCursor {
                            tree: ti,
                            tier: tier_i,
                            col,
                        };
                    let flashing = self
                        .flash
                        .as_ref()
                        .is_some_and(|(f, until)| f == id && self.tick < *until);
                    let lit = if flashing {
                        BoxLit::Flash
                    } else if is_cursor {
                        BoxLit::Cursor
                    } else {
                        BoxLit::Plain
                    };
                    self.draw_box(buf, (x, y, box_w), t, lit);
                }
                y += 2;
            }
        }
    }

    /// One talent's box at `(x, y)`, `w` wide: the icon and name over the rank meter.
    fn draw_box(&self, buf: &mut Buffer, at: (u16, u16, u16), t: &Talent, lit: BoxLit) {
        let (x, y, w) = at;
        let (c, ch) = (self.content, self.character);
        let rank = ch.talent_rank(&t.id);
        let reach = reach(c, ch, t);
        let name_fg = match reach {
            Reach::Affordable => CYAN,
            Reach::Maxed => MAGENTA,
            Reach::Out => DIM,
        };
        let fill_fg = if reach == Reach::Maxed { MAGENTA } else { CYAN };
        let name: String = format!("{} {}", t.icon, t.name)
            .chars()
            .take(w as usize)
            .collect();
        let max_x = x + w;
        put(buf, x, y, &name, Style::default().fg(name_fg), max_x);
        let filled = "▰".repeat(rank as usize);
        let empty = "▱".repeat(t.ranks.saturating_sub(rank) as usize);
        put(buf, x, y + 1, &filled, Style::default().fg(fill_fg), max_x);
        put(
            buf,
            x + rank as u16,
            y + 1,
            &empty,
            Style::default().fg(FAINT),
            max_x,
        );
        put(
            buf,
            x + t.ranks as u16,
            y + 1,
            &format!(" {rank}/{}", t.ranks),
            Style::default().fg(name_fg),
            max_x,
        );
        if lit == BoxLit::Flash {
            for cy in y..y + 2 {
                for cx in x..max_x {
                    buf[(cx, cy)].set_bg(MAGENTA).set_fg(PANEL_BG);
                }
            }
        } else if lit == BoxLit::Cursor {
            for cy in y..y + 2 {
                for cx in x..max_x {
                    let cell = &mut buf[(cx, cy)];
                    cell.set_bg(BAR_BG);
                    if cell.fg == Color::Reset {
                        cell.set_fg(TEXT);
                    }
                }
            }
            for cx in x..max_x {
                buf[(cx, y)].modifier |= Modifier::BOLD;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::{Stat, TemplateId};

    fn setup() -> (Content, Character) {
        let c = Content::load().unwrap();
        let ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        (c, ch)
    }

    #[test]
    fn the_cursor_steps_along_across_and_down_and_wraps() {
        let (c, _) = setup();
        let start = TreeCursor::default();
        let one = step(&c, start, 1, 0);
        assert_eq!(
            one,
            TreeCursor {
                tree: 0,
                tier: 0,
                col: 1
            }
        );
        let next_tree = step(&c, one, 1, 0);
        assert_eq!(
            next_tree,
            TreeCursor {
                tree: 1,
                tier: 0,
                col: 0
            }
        );
        let back = step(&c, next_tree, -1, 0);
        assert_eq!(back, one);
        let around = step(&c, start, -1, 0);
        assert_eq!(
            around,
            TreeCursor {
                tree: 4,
                tier: 0,
                col: 1
            }
        );
        let up = step(&c, start, 0, -1);
        assert_eq!(
            up,
            TreeCursor {
                tree: 0,
                tier: 4,
                col: 0
            },
            "wraps to the capstone"
        );
        let down = step(&c, up, 0, 1);
        assert_eq!(down, start);
        // Landing on a narrower tier keeps the nearest column.
        let col1 = TreeCursor {
            tree: 0,
            tier: 3,
            col: 1,
        };
        assert_eq!(step(&c, col1, 0, 1).col, 0);
        assert_eq!(
            TreeCursor::unpack(
                TreeCursor {
                    tree: 3,
                    tier: 2,
                    col: 1
                }
                .pack()
            ),
            TreeCursor {
                tree: 3,
                tier: 2,
                col: 1
            }
        );
    }

    #[test]
    fn the_dossier_reads_the_rank_the_next_and_the_reason() {
        let (c, mut ch) = setup();
        let id = c.tree(Stat::Body).unwrap().tiers[0][0].clone();
        let text = |lines: Vec<Line<'static>>| -> Vec<String> {
            lines.iter().map(|l| l.to_string()).collect()
        };
        let lines = text(dossier(&c, &ch, &id));
        assert_eq!(lines[0], "THICK SKIN · MEAT · tier 1 · rank 0/3");
        assert_eq!(lines[1], "next: +3 max HP");
        assert_eq!(lines[2], "now: nothing yet");
        ch.learn(&c, &id).unwrap();
        let lines = text(dossier(&c, &ch, &id));
        assert_eq!(lines[1], "locked · no points left · next: +3 max HP");
        assert_eq!(lines[2], "now: +3 max HP");
        ch.talents.insert(id.clone(), 3);
        assert_eq!(text(dossier(&c, &ch, &id))[1], "at its best");
    }
}
