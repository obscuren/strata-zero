//! The talents board in the deck: one tree at a time, drawn as a circuit board. Talents
//! are chips carrying their icons, prerequisites are traces, ranks are LEDs, and a rail
//! down the left lights each tier as it opens. The trees sit in slots down the left and a
//! readout on the right describes the chip under the cursor.

use crate::screens::create::attribute_rgb;
use crate::ui::art::grey;
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINT, FAINTER, MAGENTA, PANEL_BG, PANEL_BG_RGB, RULE, TEXT, put_paragraph,
    reveal,
};
use game::{Character, Content, TALENT_ART_H, TALENT_ART_W, Talent, TalentId, Tree};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Ticks the readout takes to type itself out.
const REVEAL_TICKS: u64 = 20;

// The board's layout, in cells from the body's top-left.
/// The tree slots take columns 0 to 12.
const SLOTS_W: u16 = 13;
/// The board's first column and its width.
const BOARD_X: u16 = 14;
const BOARD_W: u16 = 51;
/// The power rail.
const RAIL_X: u16 = 15;
/// The first column's chip, and the step to the next column.
const CHIP_X: u16 = 21;
const CHIP_PITCH: u16 = 11;
/// The first tier's chip row, and the step to the next tier: three rows and a gap.
const CHIP_Y: u16 = 2;
const TIER_STEP: u16 = 4;
/// The readout's first column.
const READ_X: u16 = 67;

/// Unlit rail, traces and pins.
const DARK: Color = Color::Rgb(58, 66, 80);
/// A maxed LED, and `at its best`.
const AMBER: Color = Color::Rgb(255, 200, 80);
/// The next rank's LED when a point would go in, and a met need.
const GREEN: Color = Color::Rgb(80, 230, 120);
/// An unmet need.
const RED: Color = Color::Rgb(255, 77, 106);
/// The cursor and the readout's name.
const WHITE: Color = Color::Rgb(242, 248, 255);

/// Where the cursor sits: a tree, a tier, a column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeCursor {
    pub tree: usize,
    pub tier: usize,
    pub col: usize,
}

/// The talent under a cursor.
pub fn talent_at(c: &Content, cur: TreeCursor) -> Option<&TalentId> {
    c.trees.get(cur.tree)?.slot(cur.tier, cur.col)
}

/// The tab's state: the tree on show and where the cursor last sat in each tree, as
/// `(tier, col)`. A spot that holds no talent stands for the talent nearest it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TalentsState {
    pub tree: usize,
    pub spots: Vec<(usize, usize)>,
}

impl TalentsState {
    /// The cursor: the tree on show at its spot, snapped to the nearest talent (the same
    /// tier first, then the nearest column, the left one on a tie).
    pub fn cursor(&self, c: &Content) -> TreeCursor {
        let Some(last) = c.trees.len().checked_sub(1) else {
            return TreeCursor::default();
        };
        let tree = self.tree.min(last);
        let (tier, col) = self.spots.get(tree).copied().unwrap_or((0, 0));
        let at = c.trees[tree]
            .talents()
            .min_by_key(|(t, cl, _)| (t.abs_diff(tier), cl.abs_diff(col), *cl))
            .map_or((0, 0), |(t, cl, _)| (t, cl));
        TreeCursor {
            tree,
            tier: at.0,
            col: at.1,
        }
    }

    /// Moves along the tier for `dx` (the next talent that way, wrapping) and between
    /// tiers for `dy` (the next tier that way with a talent, wrapping, landing on the
    /// talent nearest the column, the left one on a tie). Never leaves the tree.
    pub fn step(&mut self, c: &Content, dx: i32, dy: i32) {
        let cur = self.cursor(c);
        let Some(tree) = c.trees.get(cur.tree) else {
            return;
        };
        let mut at = (cur.tier, cur.col);
        if dx != 0 {
            let row: Vec<usize> = cols_in(tree, at.0);
            if let Some(i) = row.iter().position(|&cl| cl == at.1) {
                let n = row.len() as i32;
                at.1 = row[(i as i32 + dx.signum()).rem_euclid(n) as usize];
            }
        }
        if dy != 0 {
            let n = tree.tiers.len() as i32;
            let mut tier = at.0 as i32;
            for _ in 0..n {
                tier = (tier + dy.signum()).rem_euclid(n);
                let row = cols_in(tree, tier as usize);
                if let Some(&cl) = row.iter().min_by_key(|&&cl| (cl.abs_diff(at.1), cl)) {
                    at = (tier as usize, cl);
                    break;
                }
            }
        }
        self.set_spot(cur.tree, at);
    }

    /// Shows the next tree for `d` > 0, the previous for `d` < 0, wrapping; the cursor lands
    /// where it last sat in that tree.
    pub fn switch(&mut self, c: &Content, d: i32) {
        let n = c.trees.len() as i32;
        if n == 0 {
            return;
        }
        let cur = self.cursor(c);
        self.set_spot(cur.tree, (cur.tier, cur.col));
        self.tree = (cur.tree as i32 + d.signum()).rem_euclid(n) as usize;
    }

    fn set_spot(&mut self, tree: usize, at: (usize, usize)) {
        if self.spots.len() <= tree {
            self.spots.resize(tree + 1, (0, 0));
        }
        self.spots[tree] = at;
        self.tree = tree;
    }
}

/// The columns of a tier that hold a talent, left to right.
fn cols_in(tree: &Tree, tier: usize) -> Vec<usize> {
    tree.tiers
        .get(tier)
        .map(|row| {
            row.iter()
                .enumerate()
                .filter_map(|(cl, id)| id.as_ref().map(|_| cl))
                .collect()
        })
        .unwrap_or_default()
}

/// How a chip reads: maxed, open (its tier open and its prerequisite maxed), or out of
/// reach. Open says nothing of points: those decide only the green LED and the key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Maxed,
    Open,
    Out,
}

fn reach(c: &Content, ch: &Character, t: &Talent) -> Reach {
    if ch.talent_rank(&t.id) >= t.ranks {
        return Reach::Maxed;
    }
    let tier_open = c
        .tree_of(&t.id)
        .is_some_and(|(tree, tier, _)| ch.tier_open(c, tree.attribute, tier));
    let req_met = t
        .requires
        .as_ref()
        .and_then(|r| c.talent(r))
        .is_none_or(|r| ch.talent_rank(&r.id) >= r.ranks);
    if tier_open && req_met {
        Reach::Open
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

/// `text` broken into lines of at most `w` characters, at spaces where it can.
fn wrap(text: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let need = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if !line.is_empty() && need > w {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
        while line.chars().count() > w {
            let head: String = line.chars().take(w).collect();
            line = line.chars().skip(w).collect();
            out.push(head);
        }
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

/// A labelled block: `label` in its six columns, then `text` wrapped beside it, each
/// following line indented to match.
fn labelled(label: &str, text: &str, style: Style, w: usize) -> Vec<Line<'static>> {
    let dim = Style::default().fg(DIM).add_modifier(Modifier::BOLD);
    wrap(text, w.saturating_sub(6))
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let head = if i == 0 {
                format!("{label:<6}")
            } else {
                " ".repeat(6)
            };
            Line::from(vec![Span::styled(head, dim), Span::styled(l, style)])
        })
        .collect()
}

/// The readout on a talent, `w` columns wide: its file name, LEDs and tier; its name and
/// description; what the next rank adds and what the ranks held add; what it needs; the
/// line it sits in.
pub fn readout(c: &Content, ch: &Character, id: &TalentId, w: usize) -> Vec<Line<'static>> {
    let Some(t) = c.talent(id) else {
        return Vec::new();
    };
    let Some((tree, tier, _)) = c.tree_of(id) else {
        return Vec::new();
    };
    let colour = attribute_rgb(tree.attribute).to_color();
    let rank = ch.talent_rank(id);
    let style = |fg: Color| Style::default().fg(fg);
    let mut lines = vec![Line::from(Span::styled(
        format!("> {}.tal", t.id),
        style(colour).add_modifier(Modifier::BOLD),
    ))];
    let leds: String = (0..t.ranks)
        .map(|r| if r < rank { '●' } else { '○' })
        .collect();
    lines.push(Line::from(Span::styled(
        format!("  {leds} {rank}/{} · tier {}", t.ranks, tier + 1),
        style(DIM),
    )));
    lines.push(Line::from(Span::styled("─".repeat(w), style(RULE))));
    lines.push(Line::from(Span::styled(
        t.name.to_uppercase(),
        style(WHITE).add_modifier(Modifier::BOLD),
    )));
    for l in wrap(&t.desc, w) {
        lines.push(Line::from(Span::styled(l, style(FAINT))));
    }
    lines.push(Line::default());
    if rank < t.ranks {
        lines.extend(labelled(
            "NEXT",
            &words(c, t, rank as usize),
            style(TEXT),
            w,
        ));
    }
    let held: Vec<String> = (0..rank as usize)
        .map(|r| words(c, t, r))
        .filter(|s| !s.is_empty())
        .collect();
    if !held.is_empty() {
        lines.extend(labelled("NOW", &held.join(" · "), style(FAINT), w));
    }
    if rank < t.ranks {
        let mut needs: Vec<(bool, String)> = Vec::new();
        let gate = c.creation.tier_gate * tier as u32;
        if gate > 0 {
            let have = ch.tree_points(c, tree.attribute);
            needs.push(if have >= gate {
                (true, format!("{gate} points in {}", tree.name))
            } else {
                (
                    false,
                    format!("{gate} points in {} · {} more", tree.name, gate - have),
                )
            });
        }
        if let Some(r) = t.requires.as_ref().and_then(|r| c.talent(r)) {
            needs.push((
                ch.talent_rank(&r.id) >= r.ranks,
                format!("{} {}/{}", r.name, r.ranks, r.ranks),
            ));
        }
        if ch.talent_points == 0 {
            needs.push((false, "no points left".into()));
        }
        for (i, (ok, what)) in needs.into_iter().enumerate() {
            let mark = if ok { "✓" } else { "✗" };
            let label = if i == 0 { "NEED" } else { "" };
            lines.extend(labelled(
                label,
                &format!("{mark} {what}"),
                style(if ok { GREEN } else { RED }),
                w,
            ));
        }
    }
    lines.push(Line::default());
    for (i, link) in chain(c, tree, t).into_iter().enumerate() {
        let here = link.id == t.id;
        let lit = ch.talent_rank(&link.id) >= link.ranks;
        let fg = if here {
            WHITE
        } else if lit {
            colour
        } else {
            FAINT
        };
        let mut s = style(fg);
        if here {
            s = s.add_modifier(Modifier::BOLD);
        }
        lines.push(Line::from(vec![
            Span::styled(
                if i == 0 { "LINE  " } else { "      " },
                style(DIM).add_modifier(Modifier::BOLD),
            ),
            Span::styled(if here { "▸ " } else { "  " }, s),
            Span::styled(link.name.clone(), s),
        ]));
    }
    lines
}

/// The line a talent sits in: up through what it needs, down through what needs it while
/// one talent does.
fn chain<'a>(c: &'a Content, tree: &Tree, t: &'a Talent) -> Vec<&'a Talent> {
    let mut up = Vec::new();
    let mut cur = t;
    while let Some(r) = cur.requires.as_ref().and_then(|r| c.talent(r)) {
        up.push(r);
        cur = r;
    }
    up.reverse();
    up.push(t);
    let mut cur = t;
    loop {
        let kids: Vec<&Talent> = tree
            .talents()
            .filter_map(|(_, _, id)| c.talent(id))
            .filter(|k| k.requires.as_ref() == Some(&cur.id))
            .collect();
        let [kid] = kids.as_slice() else {
            break;
        };
        up.push(kid);
        cur = kid;
    }
    up
}

/// The board drawn across the deck's body: the trees' slots, the tree on show as a
/// circuit board, and the readout of the chip under the cursor.
pub struct TalentsView<'a> {
    pub content: &'a Content,
    pub character: &'a Character,
    pub cursor: TreeCursor,
    pub tick: u64,
    /// Tick the cursor last moved: the readout types itself out from there.
    pub changed: u64,
    /// A chip lit in magenta until its tick: the rank just learned.
    pub flash: Option<(TalentId, u64)>,
}

impl TalentsView<'_> {
    pub fn draw(&self, area: Rect, buf: &mut Buffer) {
        let c = self.content;
        let area = area.intersection(buf.area);
        let Some(tree) = c.trees.get(self.cursor.tree) else {
            return;
        };
        // The deck is glass; the board paints its own ground.
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_bg(PANEL_BG);
            }
        }
        let mut pen = Board { buf, area };
        self.draw_slots(&mut pen);
        self.draw_board(&mut pen, tree);
        self.draw_readout(&mut pen);
    }

    /// The trees down the left: name and points, a meter to the capstone, the points to
    /// spend under them.
    fn draw_slots(&self, pen: &mut Board) {
        let (c, ch) = (self.content, self.character);
        let bold = Modifier::BOLD;
        pen.text(1, 0, "TREES", DIM, None, bold);
        pen.text(8, 0, "[ ]", FAINTER, None, Modifier::empty());
        let mut y = 2;
        for (i, tree) in c.trees.iter().enumerate() {
            let rgb = attribute_rgb(tree.attribute);
            let on = i == self.cursor.tree;
            let pts = ch.tree_points(c, tree.attribute);
            // The points that open this tree's last tier: the meter fills toward them.
            let cap = (c.creation.tier_gate * tree.tiers.len().saturating_sub(1) as u32).max(1);
            if on {
                let ground = PANEL_BG_RGB.mix(rgb, 0.22).to_color();
                for x in 0..SLOTS_W {
                    pen.ground(x, y, ground);
                    pen.ground(x, y + 1, ground);
                }
                pen.text(0, y, "▶", rgb.to_color(), None, bold);
            }
            let name_fg = if on {
                rgb.to_color()
            } else {
                rgb.mix(PANEL_BG_RGB, 0.3).to_color()
            };
            pen.text(
                1,
                y,
                &tree.name,
                name_fg,
                None,
                if on { bold } else { Modifier::empty() },
            );
            let p = pts.to_string();
            pen.text(
                12 - p.len() as u16,
                y,
                &p,
                if on { WHITE } else { FAINT },
                None,
                if on { bold } else { Modifier::empty() },
            );
            let fill = (pts * 6 / cap).min(6) as usize;
            let meter_fg = if on {
                rgb.to_color()
            } else {
                rgb.mix(PANEL_BG_RGB, 0.4).to_color()
            };
            pen.text(
                1,
                y + 1,
                &"▰".repeat(fill),
                meter_fg,
                None,
                Modifier::empty(),
            );
            pen.text(
                1 + fill as u16,
                y + 1,
                &"▱".repeat(6 - fill),
                FAINTER,
                None,
                Modifier::empty(),
            );
            pen.text(
                8,
                y + 1,
                &format!("/{cap}"),
                FAINTER,
                None,
                Modifier::empty(),
            );
            y += 3;
        }
        let left = ch.talent_points;
        pen.text(1, y + 1, &format!("{left} to burn"), CYAN, None, bold);
    }

    /// The tree on show: ground, title, rail, traces, chips, cursor.
    fn draw_board(&self, pen: &mut Board, tree: &Tree) {
        let (c, ch) = (self.content, self.character);
        let rgb = attribute_rgb(tree.attribute);
        let colour = rgb.to_color();
        let ground = PANEL_BG_RGB.mix(rgb, 0.035).to_color();
        let dots = PANEL_BG_RGB.mix(rgb, 0.14).to_color();
        let plate = PANEL_BG_RGB.mix(rgb, 0.16).to_color();
        for y in 0..pen.area.height {
            for x in BOARD_X..BOARD_X + BOARD_W {
                pen.ground(x, y, ground);
                if (x - BOARD_X) % 4 == 2 && y % 2 == 1 {
                    pen.text(x, y, "·", dots, Some(ground), Modifier::empty());
                }
            }
        }
        pen.text(
            BOARD_X + 1,
            0,
            &tree.name,
            colour,
            Some(ground),
            Modifier::BOLD,
        );
        // The line, cut where the board ends so it never runs into the readout.
        let line_x = BOARD_X + 2 + tree.name.chars().count() as u16;
        let room = (BOARD_X + BOARD_W).saturating_sub(line_x + 1) as usize;
        let line: String = format!("· {}", tree.line).chars().take(room).collect();
        pen.text(line_x, 0, &line, FAINT, Some(ground), Modifier::empty());

        let chip = |tier: usize, col: usize| -> (u16, u16) {
            (
                CHIP_X + CHIP_PITCH * col as u16,
                CHIP_Y + TIER_STEP * tier as u16,
            )
        };

        // The rail: lit down to the last open tier's tap.
        let tiers = tree.tiers.len();
        let open_to = (0..tiers)
            .filter(|&t| ch.tier_open(c, tree.attribute, t))
            .max()
            .unwrap_or(0);
        let lit_to = chip(open_to, 0).1 + 1;
        let bottom = chip(tiers.saturating_sub(1), 0).1 + 2;
        for y in CHIP_Y - 1..=bottom {
            let fg = if y <= lit_to { colour } else { DARK };
            pen.text(RAIL_X, y, "┃", fg, Some(ground), Modifier::empty());
        }
        for t in 0..tiers {
            let y = chip(t, 0).1 + 1;
            let open = ch.tier_open(c, tree.attribute, t);
            let fg = if open { colour } else { DARK };
            pen.text(RAIL_X, y, "┣━", fg, Some(ground), Modifier::empty());
            let need = c.creation.tier_gate * t as u32;
            pen.text(
                RAIL_X + 2,
                y,
                &format!("{need:>2}"),
                if open { colour } else { FAINTER },
                Some(ground),
                if open {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                },
            );
        }

        // Traces, under the chips: across the prerequisite's tier, then down.
        for (tier, col, id) in tree.talents() {
            let Some(t) = c.talent(id) else { continue };
            let Some(req) = t.requires.as_ref().and_then(|r| c.talent(r)) else {
                continue;
            };
            let Some((rtier, rcol, _)) = tree.talents().find(|(_, _, r)| *r == &req.id) else {
                continue;
            };
            let fg = if ch.talent_rank(&req.id) >= req.ranks {
                colour
            } else {
                DARK
            };
            let (xs, ys) = chip(rtier, rcol);
            let (xt, yt) = chip(tier, col);
            let x = xt + 2;
            let mut y = ys + 3;
            if rcol != col {
                let mid = ys + 1;
                if col < rcol {
                    pen.text(x, mid, "┌", fg, Some(ground), Modifier::empty());
                    for hx in x + 1..xs - 2 {
                        pen.text(hx, mid, "─", fg, Some(ground), Modifier::empty());
                    }
                    pen.text(xs - 2, mid, "●", fg, Some(ground), Modifier::empty());
                } else {
                    pen.text(xs + 8, mid, "●", fg, Some(ground), Modifier::empty());
                    for hx in xs + 9..x {
                        pen.text(hx, mid, "─", fg, Some(ground), Modifier::empty());
                    }
                    pen.text(x, mid, "┐", fg, Some(ground), Modifier::empty());
                }
                y = mid + 1;
            }
            while y + 1 < yt {
                pen.text(x, y, "│", fg, Some(ground), Modifier::empty());
                y += 1;
            }
            pen.text(x, yt - 1, "▼", fg, Some(ground), Modifier::empty());
        }

        // The chips.
        for (tier, col, id) in tree.talents() {
            let Some(t) = c.talent(id) else { continue };
            let (x, y) = chip(tier, col);
            let rank = ch.talent_rank(id);
            let state = reach(c, ch, t);
            let learnable = ch.can_learn(c, id).is_ok();
            let flashing = self
                .flash
                .as_ref()
                .is_some_and(|(f, until)| f == id && self.tick < *until);
            let lit = rank > 0 || state != Reach::Out;
            if let Some(art) = c.talent_art(id) {
                pen.icon(x, y, art, plate, lit);
            }
            let pin = if flashing {
                MAGENTA
            } else if rank > 0 {
                colour
            } else if state == Reach::Open {
                DIM
            } else {
                DARK
            };
            for j in 0..3 {
                pen.text(x - 1, y + j, "┤", pin, Some(ground), Modifier::empty());
                pen.text(x + 6, y + j, "├", pin, Some(ground), Modifier::empty());
            }
            for r in 0..t.ranks {
                let on = r < rank;
                let fg = if flashing {
                    MAGENTA
                } else if on && state == Reach::Maxed {
                    AMBER
                } else if on {
                    colour
                } else if learnable && r == rank {
                    GREEN
                } else {
                    FAINTER
                };
                let modi = if on {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                };
                pen.text(
                    x + 7,
                    y + 2 - r as u16,
                    if on { "●" } else { "○" },
                    fg,
                    Some(ground),
                    modi,
                );
            }
        }

        // The cursor's corners, in the gaps around its chip.
        let (x, y) = chip(self.cursor.tier, self.cursor.col);
        for (cx, cy, s) in [
            (x - 2, y - 1, "┏"),
            (x + 8, y - 1, "┓"),
            (x - 2, y + 3, "┗"),
            (x + 8, y + 3, "┛"),
        ] {
            pen.text(cx, cy, s, WHITE, Some(ground), Modifier::BOLD);
        }
    }

    /// The readout of the chip under the cursor, typing itself out; the key on its last
    /// row.
    fn draw_readout(&self, pen: &mut Board) {
        let (c, ch) = (self.content, self.character);
        let Some(id) = talent_at(c, self.cursor) else {
            return;
        };
        let w = pen.area.width.saturating_sub(READ_X + 1);
        if w < 8 || pen.area.height < 2 {
            return;
        }
        let lines = readout(c, ch, id, w as usize);
        let p = self.tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
        let shown = reveal(&lines, p, (self.tick / 4).is_multiple_of(2));
        let rect = Rect::new(pen.area.x + READ_X, pen.area.y, w, pen.area.height - 1);
        put_paragraph(pen.buf, rect, shown);
        let last = pen.area.height - 1;
        let t = c.talent(id);
        if ch.can_learn(c, id).is_ok() {
            pen.text(READ_X, last, "[space]", WHITE, Some(BAR_BG), Modifier::BOLD);
            pen.text(
                READ_X + 8,
                last,
                "burn a point",
                TEXT,
                None,
                Modifier::empty(),
            );
        } else if t.is_some_and(|t| ch.talent_rank(id) >= t.ranks) {
            pen.text(READ_X, last, "at its best", AMBER, None, Modifier::empty());
        }
    }
}

/// Writes inside the body, by offsets from its top-left, nothing outside it.
struct Board<'b> {
    buf: &'b mut Buffer,
    area: Rect,
}

impl Board<'_> {
    fn cell(&mut self, x: u16, y: u16) -> Option<&mut ratatui::buffer::Cell> {
        if x >= self.area.width || y >= self.area.height {
            return None;
        }
        Some(&mut self.buf[(self.area.x + x, self.area.y + y)])
    }

    fn ground(&mut self, x: u16, y: u16, bg: Color) {
        if let Some(cell) = self.cell(x, y) {
            cell.set_bg(bg);
        }
    }

    /// `s` from (x, y) in `fg` on `bg` (or on what is there), with `modifier`.
    fn text(&mut self, x: u16, y: u16, s: &str, fg: Color, bg: Option<Color>, modifier: Modifier) {
        for (i, ch) in s.chars().enumerate() {
            if let Some(cell) = self.cell(x + i as u16, y) {
                cell.set_char(ch).set_fg(fg);
                cell.modifier = modifier;
                if let Some(bg) = bg {
                    cell.set_bg(bg);
                }
            }
        }
    }

    /// A talent's icon on its plate, three cells tall, greyed when `lit` is false.
    fn icon(&mut self, x: u16, y: u16, art: &game::TalentArt, plate: Color, lit: bool) {
        let px = |row: usize, col: usize| -> Color {
            let ch = art
                .rows
                .get(row)
                .and_then(|r| r.chars().nth(col))
                .unwrap_or('.');
            let c = art
                .colours
                .get(&ch)
                .filter(|_| ch != '.')
                .map_or(plate, |&(r, g, b)| Color::Rgb(r, g, b));
            if lit { c } else { grey(c, PANEL_BG) }
        };
        for cy in 0..TALENT_ART_H / 2 {
            for cx in 0..TALENT_ART_W {
                let (top, bottom) = (px(2 * cy, cx), px(2 * cy + 1, cx));
                if let Some(cell) = self.cell(x + cx as u16, y + cy as u16) {
                    cell.modifier = Modifier::empty();
                    if top == bottom {
                        cell.set_char(' ').set_fg(top).set_bg(top);
                    } else {
                        cell.set_char('▀').set_fg(top).set_bg(bottom);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::TemplateId;

    fn setup() -> (Content, Character) {
        let c = Content::load().unwrap();
        let ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        (c, ch)
    }

    fn tree_index(c: &Content, name: &str) -> usize {
        c.trees.iter().position(|t| t.name == name).unwrap()
    }

    fn on(c: &Content, s: &TalentsState) -> String {
        talent_at(c, s.cursor(c)).unwrap().0.clone()
    }

    fn edge(c: &Content) -> TalentsState {
        TalentsState {
            tree: tree_index(c, "EDGE"),
            spots: Vec::new(),
        }
    }

    #[test]
    fn along_a_tier_skips_the_gaps_and_wraps() {
        let (c, _) = setup();
        let mut s = edge(&c);
        assert_eq!(on(&c, &s), "quick_draw");
        s.step(&c, 1, 0);
        assert_eq!(on(&c, &s), "steady_hand");
        s.step(&c, 1, 0);
        assert_eq!(on(&c, &s), "light_step", "the empty slot is skipped");
        s.step(&c, 1, 0);
        assert_eq!(on(&c, &s), "quick_draw", "wraps within the tier");
        s.step(&c, -1, 0);
        assert_eq!(on(&c, &s), "light_step");
        assert_eq!(
            s.cursor(&c).tree,
            tree_index(&c, "EDGE"),
            "never leaves the tree"
        );
    }

    #[test]
    fn up_and_down_find_the_nearest_talent() {
        let (c, _) = setup();
        let mut s = edge(&c);
        s.step(&c, -1, 0);
        assert_eq!(on(&c, &s), "light_step");
        for want in [
            "backstab",
            "counter_cut",
            "razor_wire",
            "half_second",
            "steady_hand",
        ] {
            s.step(&c, 0, 1);
            assert_eq!(on(&c, &s), want);
        }
        s.step(&c, 0, -1);
        assert_eq!(on(&c, &s), "half_second", "up wraps to the capstone");
        // From Tinker (column 1), Live wire (0) and Chrome whisperer (2) are as near:
        // the left one wins.
        let w = tree_index(&c, "WRENCH");
        let mut spots = vec![(0, 0); w + 1];
        spots[w] = (2, 1);
        let mut s = TalentsState { tree: w, spots };
        assert_eq!(on(&c, &s), "tinker");
        s.step(&c, 0, 1);
        assert_eq!(on(&c, &s), "live_wire");
    }

    #[test]
    fn trees_switch_and_remember() {
        let (c, _) = setup();
        let mut s = TalentsState::default();
        assert_eq!(on(&c, &s), "thick_skin");
        s.switch(&c, 1);
        assert_eq!(on(&c, &s), "quick_draw");
        s.step(&c, 0, 1);
        s.step(&c, 0, 1);
        assert_eq!(on(&c, &s), "hair_trigger");
        s.switch(&c, -1);
        assert_eq!(on(&c, &s), "thick_skin");
        s.switch(&c, 1);
        assert_eq!(on(&c, &s), "hair_trigger", "EDGE remembers its spot");
        let mut s = TalentsState::default();
        s.switch(&c, -1);
        assert_eq!(s.cursor(&c).tree, tree_index(&c, "ICE"), "wraps backwards");
    }

    #[test]
    fn a_stale_spot_snaps_to_a_talent() {
        let (c, _) = setup();
        let e = tree_index(&c, "EDGE");
        let mut spots = vec![(0, 0); e + 1];
        spots[e] = (2, 3);
        let s = TalentsState { tree: e, spots };
        assert_eq!(on(&c, &s), "counter_cut");
        let mut s = s;
        s.spots[e] = (9, 9);
        assert_eq!(on(&c, &s), "half_second");
        let s = TalentsState {
            tree: 99,
            spots: Vec::new(),
        };
        assert_eq!(s.cursor(&c).tree, c.trees.len() - 1);
    }

    #[test]
    fn the_readout_reads_the_rank_the_next_the_needs_and_the_line() {
        let (c, mut ch) = setup();
        let text = |lines: Vec<Line<'static>>| -> Vec<String> {
            lines.iter().map(|l| l.to_string()).collect()
        };
        let lines = text(readout(&c, &ch, &TalentId::new("thick_skin"), 30));
        assert_eq!(lines[0], "> thick_skin.tal");
        assert_eq!(lines[1], "  ○○○ 0/3 · tier 1");
        assert_eq!(lines[3], "THICK SKIN");
        assert!(lines.contains(&"NEXT  +3 max HP".to_string()), "{lines:?}");
        assert!(
            !lines.iter().any(|l| l.starts_with("NEED")),
            "tier 1 needs nothing: {lines:?}"
        );
        ch.talent_points = 0;
        let lines = text(readout(&c, &ch, &TalentId::new("bullet_time"), 40));
        assert!(
            lines.contains(&"NEED  ✗ 6 points in EDGE · 6 more".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"      ✗ Long shot 2/2".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"      ✗ no points left".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"LINE    Steady hand".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"      ▸ Bullet time".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"        The half second".to_string()),
            "{lines:?}"
        );
        ch.talents.insert(TalentId::new("thick_skin"), 3);
        let lines = text(readout(&c, &ch, &TalentId::new("thick_skin"), 30));
        assert_eq!(lines[1], "  ●●● 3/3 · tier 1");
        assert!(!lines.iter().any(|l| l.starts_with("NEXT")), "{lines:?}");
        assert!(
            lines.contains(&"NOW   +3 max HP · +3 max HP ·".to_string()),
            "{lines:?}"
        );
    }

    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("", 4), vec![""]);
    }
}
