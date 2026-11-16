//! The body on GEAR & INVENTORY: a tile per part, placed where the part sits on a body
//! and joined by lines the way the parts join, what a tile says, and where the arrows go
//! from one.

use super::lit_bold;
use crate::app::AppState;
use crate::screens::deck::{cut, lit};
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, PANEL_BG, RULE, TEXT, put};
use game::{BodyPart, Item, ItemCopy, PartKind, Perk};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// A deck's slots and an item's sockets on a tile, unlit.
const PIPS: Color = Color::Rgb(0x3f, 0x7c, 0x87);

/// An arrow's way across the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// A line between two joined tiles: the corners it turns at, from a point on one tile's
/// border to a point on the other's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub path: Vec<(u16, u16)>,
}

/// Where the body draws in the tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyLayout<K = BodyPart> {
    /// The full body; else the compact one, in the left half.
    pub full: bool,
    /// The columns the body takes; the divider stands just right of them.
    pub left: Rect,
    /// The parts drawn, in body order, and their tiles.
    pub tiles: Vec<(K, Rect)>,
    pub lines: Vec<Line>,
}

impl<K: Copy + Eq> BodyLayout<K> {
    pub(crate) fn tile(&self, key: K) -> Option<Rect> {
        self.tiles.iter().find(|(k, _)| *k == key).map(|(_, r)| *r)
    }
}

/// The tab's least size, in columns and rows, for the full body: its columns, the
/// divider and 50 for the pack and the card; the header, its rule and the graph.
pub(crate) const FULL_MIN: (u16, u16) = (113, 27);
/// Columns the full body takes: the graph's 60 and a column each side.
pub(crate) const FULL_W: u16 = 62;

/// Where a part's tile sits from the graph's origin, its size, the column from its left
/// edge where a line from above or below meets it, and the part its line runs to.
pub(crate) struct Place<K> {
    key: K,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    joint: u16,
    joins: Option<K>,
}

pub(crate) const fn at<K>(key: K, (x, y): (u16, u16), (w, h): (u16, u16), joint: u16) -> Place<K> {
    Place {
        key,
        x,
        y,
        w,
        h,
        joint,
        joins: None,
    }
}

impl<K: Copy> Place<K> {
    pub(crate) const fn to(self, key: K) -> Place<K> {
        Place {
            joins: Some(key),
            ..self
        }
    }
}

const SMALL: (u16, u16) = (11, 5);
const BIG: (u16, u16) = (12, 7);

/// The full body, as the mock places it, with free places for a third arm and its hand
/// under R ARM and R HAND.
const FULL: [Place<BodyPart>; 13] = {
    use BodyPart::*;
    [
        at(Head, (24, 0), SMALL, 9),
        at(Face, (36, 0), SMALL, 9).to(Head),
        at(LeftHand, (0, 7), SMALL, 9).to(LeftArm),
        at(LeftArm, (12, 7), SMALL, 9).to(Torso),
        at(Torso, (24, 6), BIG, 9).to(Head),
        at(RightArm, (37, 7), SMALL, 9).to(Torso),
        at(RightHand, (49, 7), SMALL, 9).to(RightArm),
        at(Back, (12, 14), SMALL, 8).to(Torso),
        at(Waist, (24, 14), SMALL, 9).to(Torso),
        at(Legs, (24, 20), SMALL, 9).to(Waist),
        at(Feet, (36, 20), SMALL, 9).to(Legs),
        at(ThirdArm, (37, 14), SMALL, 9).to(Torso),
        at(ThirdHand, (49, 14), SMALL, 9).to(ThirdArm),
    ]
};

const TILE: (u16, u16) = (14, 4);
const TRUNK: (u16, u16) = (14, 6);

/// The compact body: the same shape without arms, the hands on the torso, tiles 4 tall
/// for a name on one line.
const COMPACT: [Place<BodyPart>; 10] = {
    use BodyPart::*;
    [
        at(Head, (15, 0), TILE, 10),
        at(Face, (30, 0), TILE, 10).to(Head),
        at(LeftHand, (0, 6), TILE, 10).to(Torso),
        at(Torso, (15, 5), TRUNK, 10).to(Head),
        at(RightHand, (30, 6), TILE, 10).to(Torso),
        at(Back, (0, 12), TILE, 10).to(Torso),
        at(Waist, (15, 12), TILE, 10).to(Torso),
        at(Legs, (15, 17), TILE, 10).to(Waist),
        at(Feet, (30, 17), TILE, 10).to(Legs),
        at(ThirdHand, (30, 12), TILE, 10).to(Torso),
    ]
};

/// The body's side of the tab `area` for `parts`: the full body when the tab has room
/// for it, else the compact one in the left half; under its header, centred in its
/// columns and rows. A tab too small for the deck (under 100×30) leaves out the tiles
/// that would hang outside it.
pub(crate) fn layout(area: Rect, parts: &[BodyPart]) -> BodyLayout {
    place(area, &FULL, &COMPACT, parts, FULL_W)
}

/// `layout` for any table: the full places when the tab has room for them (its columns
/// `full_w`), else the compact ones in the left half, for the `keys` to draw.
pub(crate) fn place<K: Copy + Eq>(
    area: Rect,
    full: &[Place<K>],
    compact: &[Place<K>],
    keys: &[K],
    full_w: u16,
) -> BodyLayout<K> {
    let is_full = area.width >= FULL_MIN.0 && area.height >= FULL_MIN.1;
    let (table, width, header) = if is_full {
        (full, full_w, 2)
    } else {
        (compact, area.width.saturating_sub(1) / 2, 1)
    };
    let left = Rect::new(area.x, area.y, width.min(area.width), area.height);
    let graph_w = table.iter().map(|p| p.x + p.w).max().unwrap_or(0);
    let graph_h = table.iter().map(|p| p.y + p.h).max().unwrap_or(0);
    let ox = left.x + left.width.saturating_sub(graph_w) / 2;
    let oy = left.y + header + left.height.saturating_sub(header + graph_h) / 2;
    let rect = |p: &Place<K>| Rect::new(ox + p.x, oy + p.y, p.w, p.h);
    let placed = |key: K| {
        table
            .iter()
            .find(|p| p.key == key)
            .filter(|p| keys.contains(&key) && left.intersection(rect(p)) == rect(p))
    };
    let tiles = keys
        .iter()
        .filter_map(|&key| placed(key).map(|p| (key, rect(p))))
        .collect();
    let lines = keys
        .iter()
        .filter_map(|&key| {
            let child = placed(key)?;
            let parent = placed(child.joins?)?;
            Some(line(
                (rect(parent), parent.joint),
                (rect(child), child.joint),
            ))
        })
        .collect();
    BodyLayout {
        full: is_full,
        left,
        tiles,
        lines,
    }
}

/// The line between two tiles with their joints: side by side, across the gap on the
/// left one's middle row; one above the other, down from the upper one's joint and, when
/// the joints are not in line, along the row under it to the lower one's.
fn line((a, ja): (Rect, u16), (b, jb): (Rect, u16)) -> Line {
    if a.y < b.bottom() && b.y < a.bottom() {
        let (l, r) = if a.x < b.x { (a, b) } else { (b, a) };
        let y = l.y + l.height / 2;
        return Line {
            path: vec![(l.right() - 1, y), (r.x, y)],
        };
    }
    let ((t, jt), (u, ju)) = if a.y < b.y {
        ((a, ja), (b, jb))
    } else {
        ((b, jb), (a, ja))
    };
    let (x0, x1, y0) = (t.x + jt, u.x + ju, t.bottom() - 1);
    let path = if x0 == x1 {
        vec![(x0, y0), (x0, u.y)]
    } else {
        vec![(x0, y0), (x0, t.bottom()), (x1, t.bottom()), (x1, u.y)]
    };
    Line { path }
}

const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;

/// The cells the lines draw, with the glyph each takes: where a line meets a tile's
/// border, or lines meet each other, the junction of all of them.
pub(crate) fn joints<K>(l: &BodyLayout<K>) -> Vec<((u16, u16), char)> {
    let mut ways: std::collections::BTreeMap<(u16, u16), u8> = Default::default();
    for line in &l.lines {
        for pair in line.path.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            if y0 == y1 {
                let (a, b) = (x0.min(x1), x0.max(x1));
                for x in a..=b {
                    let way = if x > a { LEFT } else { 0 } | if x < b { RIGHT } else { 0 };
                    *ways.entry((x, y0)).or_default() |= way;
                }
            } else {
                let (a, b) = (y0.min(y1), y0.max(y1));
                for y in a..=b {
                    let way = if y > a { UP } else { 0 } | if y < b { DOWN } else { 0 };
                    *ways.entry((x0, y)).or_default() |= way;
                }
            }
        }
        // Each end is on a tile's border, which runs on through it.
        for &(x, y) in [line.path.first(), line.path.last()].into_iter().flatten() {
            let tile = l.tiles.iter().map(|(_, r)| r);
            let border = tile
                .filter(|r| r.contains((x, y).into()))
                .map(|r| {
                    if y == r.y || y == r.bottom() - 1 {
                        LEFT | RIGHT
                    } else {
                        UP | DOWN
                    }
                })
                .next()
                .unwrap_or(0);
            *ways.entry((x, y)).or_default() |= border;
        }
    }
    ways.into_iter().map(|(at, way)| (at, glyph(way))).collect()
}

/// The box-drawing glyph that runs each `way`.
fn glyph(way: u8) -> char {
    match way {
        w if w == UP | DOWN | LEFT | RIGHT => '┼',
        w if w == LEFT | RIGHT | DOWN => '┬',
        w if w == LEFT | RIGHT | UP => '┴',
        w if w == UP | DOWN | RIGHT => '├',
        w if w == UP | DOWN | LEFT => '┤',
        w if w == RIGHT | DOWN => '┌',
        w if w == LEFT | DOWN => '┐',
        w if w == RIGHT | UP => '└',
        w if w == LEFT | UP => '┘',
        w if w & (LEFT | RIGHT) != 0 => '─',
        _ => '│',
    }
}

/// The part an arrow goes to from `from`: among the drawn tiles whose centre lies that
/// way (no more than half as far across it as along it), the nearest along it, then the
/// nearest across. `None` when there is none.
pub(crate) fn step<K: Copy + Eq>(layout: &BodyLayout<K>, from: K, dir: Dir) -> Option<K> {
    // In half cells, so a centre is a whole number.
    let centre = |r: Rect| {
        (
            2 * i32::from(r.x) + i32::from(r.width),
            2 * i32::from(r.y) + i32::from(r.height),
        )
    };
    let (fx, fy) = centre(layout.tile(from)?);
    layout
        .tiles
        .iter()
        .filter(|(p, _)| *p != from)
        .filter_map(|&(p, r)| {
            let (x, y) = centre(r);
            let (dx, dy) = (x - fx, y - fy);
            let (along, across) = match dir {
                Dir::Up => (-dy, dx),
                Dir::Down => (dy, dx),
                Dir::Left => (-dx, dy),
                Dir::Right => (dx, dy),
            };
            (along > 0 && 2 * across.abs() <= along).then_some(((along, across.abs()), p))
        })
        .min_by_key(|(near, _)| *near)
        .map(|(_, p)| p)
}

/// How a tile's last line inks a run of what the worn item gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ink {
    Label,
    /// The main hand's `●`.
    Main,
    /// A deck's slots, an item's sockets.
    Pips,
}

/// The worn item's name in at most `rows` lines of `room` columns: wrapped at spaces,
/// a word too long for its line cut with `…`, the last line taking the rest, cut.
fn name_lines(name: &str, room: usize, rows: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut words = name.split_whitespace().peekable();
    while let Some(word) = words.next() {
        let mut line = word.to_string();
        let last = lines.len() + 1 >= rows;
        while let Some(next) = words.peek() {
            if !last && line.chars().count() + 1 + next.chars().count() > room {
                break;
            }
            line = format!("{line} {next}");
            words.next();
        }
        lines.push(cut(&line, room));
    }
    lines
}

/// What the item `part` wears gives, in a tile's `room` columns, as runs of text: a
/// hand starts with `●` (the main hand) or `○`; then a gun's damage × rounds and a pip
/// per socket (`■` fitted, `□` empty), a deck's program slots and its sockets, a cell's
/// charge (`◌ 62%`, `· burned` when burned out), or the item's first perk, short.
/// Nothing for an empty part.
fn gives(st: &AppState, part: BodyPart, room: usize) -> Vec<(String, Ink)> {
    let Some(item) = st
        .character
        .equipped(part)
        .and_then(|id| st.content.item(id))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if st
        .content
        .node(part)
        .is_some_and(|n| n.kind == PartKind::Hand)
    {
        out.push(if st.character.main_hand(&st.content) == Some(part) {
            ("● ".to_string(), Ink::Main)
        } else {
            ("○ ".to_string(), Ink::Label)
        });
    }
    let copy = st.character.worn_copy(part);
    let pips = |item: &Item| -> String {
        (0..item.sockets.len())
            .map(|i| {
                let fitted = copy.is_some_and(|k| k.fitted.get(i).is_some_and(Option::is_some));
                if fitted { '■' } else { '□' }
            })
            .collect()
    };
    let sockets = |out: &mut Vec<(String, Ink)>| {
        if !item.sockets.is_empty() {
            out.push((" ".to_string(), Ink::Label));
            out.push((pips(item), Ink::Pips));
        }
    };
    if let Some(w) = &item.weapon {
        out.push((format!("{}×{}", w.damage, w.rounds), Ink::Label));
        sockets(&mut out);
    } else if let Some(deck) = item.deck {
        out.push(("■".repeat(deck.slots as usize), Ink::Pips));
        sockets(&mut out);
    } else if item.cell
        && let Some(c) = copy.map(ItemCopy::cell)
    {
        out.push((
            if c.burned {
                "· burned".to_string()
            } else {
                format!("◌ {}%", crate::cloak::pct(c.left))
            },
            Ink::Label,
        ));
    } else if let Some(perk) = item.perks.first() {
        let used: usize = out.iter().map(|(t, _)| t.chars().count()).sum();
        out.push((perk_short(perk, room.saturating_sub(used)), Ink::Label));
    }
    out
}

/// A perk in a tile's few columns: `+5 HP`, `+4 kg`, `COOL +1` (`REF +1` when the
/// attribute's name has no room).
fn perk_short(perk: &Perk, room: usize) -> String {
    match *perk {
        Perk::Attribute(a, n) => {
            let long = format!("{} {n:+}", a.name());
            if long.chars().count() <= room {
                long
            } else {
                format!("{} {n:+}", a.short())
            }
        }
        Perk::Sight(n) => format!("{n:+} sight"),
        Perk::Carry(kg) => format!("{}{kg} kg", if kg >= 0.0 { "+" } else { "" }),
        Perk::HpMax(n) => format!("{n:+} HP"),
        Perk::Focus(n) => format!("{n:+} focus"),
        Perk::Armor(n) => format!("{n:+} armour"),
        Perk::Move(n) => format!("{n:+} move"),
        Perk::Resist(harm, n) => format!("{n:+}% {}", harm.label()),
        Perk::Throw(n) => format!("{n:+} throw"),
    }
}

/// How the cursor lights a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lit {
    Off,
    /// The cursor's tile while the body has the focus.
    On,
    /// The cursor's tile while the pack has it.
    Dim,
}

/// The body: the header (`▌MEAT▐`, lit while the body has the focus) and in the full
/// body a rule under it, then the tiles, the cursor's lit, and the lines between them.
pub(crate) fn draw(
    st: &AppState,
    l: &BodyLayout,
    cursor: BodyPart,
    focused: bool,
    buf: &mut Buffer,
) {
    let header = if focused {
        lit_bold(st.tick)
    } else {
        Style::default().fg(DIM)
    };
    put(
        buf,
        l.left.x + 1,
        l.left.y,
        "▌MEAT▐",
        header,
        l.left.right(),
    );
    if l.full {
        for x in l.left.x..l.left.right() {
            buf[(x, l.left.y + 1)].set_char('─').set_fg(RULE);
        }
    }
    let rows = if l.full { 2 } else { 1 };
    for &(part, r) in &l.tiles {
        let light = match (part == cursor, focused) {
            (false, _) => Lit::Off,
            (true, true) => Lit::On,
            (true, false) => Lit::Dim,
        };
        draw_tile(st, part, r, light, rows, buf);
    }
    for ((x, y), ch) in joints(l) {
        buf[(x, y)].set_char(ch).set_fg(RULE);
    }
}

/// One part's tile: its name in the top border; inside, the worn item's name on `rows`
/// lines and, dim under it, what it gives, or a dim `·` when nothing is worn.
fn draw_tile(st: &AppState, part: BodyPart, r: Rect, light: Lit, rows: u16, buf: &mut Buffer) {
    let (x, y, w, h) = (r.x, r.y, r.width, r.height);
    let (right, inner) = (r.right(), r.right() - 1);
    let on = light == Lit::On;
    let bg = if on { BAR_BG } else { PANEL_BG };
    let edge = Style::default().fg(match light {
        Lit::Off => RULE,
        Lit::On => CYAN,
        Lit::Dim => DIM,
    });
    let run = "─".repeat(usize::from(w - 2));
    put(buf, x, y, &format!("┌{run}┐"), edge, right);
    let label = st.content.node(part).map_or("", |n| n.name.as_str());
    let label_style = if on {
        lit_bold(st.tick)
    } else {
        Style::default().fg(DIM)
    };
    put(buf, x + 1, y, &format!(" {label} "), label_style, inner);
    for cy in y + 1..y + h - 1 {
        put(buf, x, cy, "│", edge, right);
        put(buf, inner, cy, "│", edge, right);
        for cx in x + 1..inner {
            buf[(cx, cy)].reset();
            buf[(cx, cy)].set_bg(bg);
        }
    }
    put(buf, x, y + h - 1, &format!("└{run}┘"), edge, right);

    let Some(item) = st
        .character
        .equipped(part)
        .and_then(|id| st.content.item(id))
    else {
        let dot = Style::default().fg(FAINTER).bg(bg);
        put(buf, x + w / 2, y + h / 2, "·", dot, inner);
        return;
    };
    let room = usize::from(w - 2);
    let top = y + 1 + (h - 2).saturating_sub(rows + 1) / 2;
    let mut name = Style::default().fg(TEXT).bg(bg);
    if on {
        name = name.fg(lit(st.tick)).add_modifier(Modifier::BOLD);
    }
    for (i, line) in name_lines(&item.name, room, usize::from(rows))
        .iter()
        .enumerate()
    {
        put(buf, x + 1, top + i as u16, line, name, inner);
    }
    let mut cx = x + 1;
    for (text, ink) in gives(st, part, room) {
        let fg = match ink {
            Ink::Label => DIM,
            Ink::Main => CYAN,
            Ink::Pips if on => CYAN,
            Ink::Pips => PIPS,
        };
        put(
            buf,
            cx,
            top + rows,
            &text,
            Style::default().fg(fg).bg(bg),
            inner,
        );
        cx = cx.saturating_add(text.chars().count() as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BodyPart::*;

    const DEFAULT: [BodyPart; 11] = [
        Head, Face, Back, Torso, RightArm, RightHand, LeftArm, LeftHand, Waist, Legs, Feet,
    ];

    fn third() -> Vec<BodyPart> {
        DEFAULT
            .iter()
            .copied()
            .chain([ThirdArm, ThirdHand])
            .collect()
    }

    /// The tab's body for a window of `w` by `h`, as the deck sets it.
    fn tab(w: u16, h: u16) -> Rect {
        let top = if h >= 36 { 10 } else { 5 };
        Rect::new(1, top, w - 2, h - top - 3)
    }

    #[test]
    fn the_footer_and_the_back_tile_read_the_worn_charge() {
        use crate::cloak::hud::footer_charge;
        use crate::cloak::testutil::{scrambler, set_charge, spare_scrambler, street};
        use game::{Charge, FULL_CELL};
        let text = |runs: Vec<(String, Ink)>| -> String { runs.into_iter().map(|r| r.0).collect() };
        let footer = |st: &AppState| -> String {
            footer_charge(st)
                .iter()
                .map(|s| s.content.as_ref())
                .collect()
        };
        let mut st = street();
        let k = scrambler(&mut st);
        let at = |pct: u32, burned: bool| Charge {
            left: FULL_CELL * pct / 100,
            burned,
        };
        set_charge(&mut st, k, at(62, false));
        assert_eq!(footer(&st), "◌ field scrambler  62%");
        assert_eq!(text(gives(&st, Back, 9)), "◌ 62%");
        set_charge(&mut st, k, at(5, true));
        assert_eq!(text(gives(&st, Back, 9)), "· burned");

        let mut st = street();
        let k = scrambler(&mut st);
        let spare = spare_scrambler(&mut st);
        set_charge(&mut st, k, at(62, false));
        set_charge(&mut st, spare, at(90, false));
        assert_eq!(
            footer(&st),
            "◌ field scrambler  62%",
            "the worn one, not the spare"
        );
        assert_eq!(text(gives(&st, Back, 9)), "◌ 62%");

        let mut st = street();
        spare_scrambler(&mut st);
        assert_eq!(footer(&st), "", "one in the pack shows nothing");
        assert!(gives(&st, Back, 9).iter().all(|r| !r.0.contains('◌')));
    }

    #[test]
    fn a_name_wraps_at_spaces_and_the_last_line_is_cut() {
        assert_eq!(
            name_lines("Rebuilt pawn deck", 9, 2),
            ["Rebuilt", "pawn deck"]
        );
        assert_eq!(
            name_lines("Hardhat with visor", 9, 2),
            ["Hardhat", "with vis…"]
        );
        assert_eq!(name_lines("Mirrorshades", 9, 2), ["Mirrorsh…"]);
        assert_eq!(name_lines("Ratchet SMG", 12, 1), ["Ratchet SMG"]);
        assert_eq!(name_lines("Monowire shiv", 12, 1), ["Monowire sh…"]);
    }

    #[test]
    fn below_113_columns_or_27_rows_the_body_goes_compact() {
        for (w, h) in [(112, 27), (113, 26)] {
            let l = layout(Rect::new(1, 5, w, h), &DEFAULT);
            assert!(!l.full, "{w}x{h}");
            assert!(l.tile(LeftArm).is_none() && l.tile(RightArm).is_none());
            for (part, r) in &l.tiles {
                let tall = if *part == Torso { 6 } else { 4 };
                assert_eq!(r.height, tall, "{part:?} at {w}x{h}");
            }
        }
        assert!(layout(Rect::new(1, 5, 113, 27), &DEFAULT).full);
        let l = layout(tab(100, 30), &DEFAULT);
        assert!(!l.full);
        assert_eq!(l.left, Rect::new(1, 5, 48, 22));
        for (part, r) in &l.tiles {
            assert!(l.left.contains((r.x, r.y).into()), "{part:?} {r:?}");
            assert!(r.right() <= l.left.right() && r.bottom() <= l.left.bottom());
        }
    }

    #[test]
    fn the_full_body_sits_where_the_mock_puts_it() {
        let l = layout(tab(120, 40), &DEFAULT);
        assert!(l.full);
        assert_eq!(l.left, Rect::new(1, 10, 62, 27));
        assert_eq!(l.tile(Head), Some(Rect::new(26, 12, 11, 5)));
        assert_eq!(l.tile(Torso), Some(Rect::new(26, 18, 12, 7)));
        assert_eq!(l.tile(RightHand), Some(Rect::new(51, 19, 11, 5)));
        assert_eq!(l.tile(Feet), Some(Rect::new(38, 32, 11, 5)));
        assert!(l.tile(ThirdArm).is_none(), "not in the body");
    }

    #[test]
    fn every_size_draws_without_overlap() {
        let sizes = [
            (100, 30),
            (112, 30),
            (113, 40),
            (120, 40),
            (160, 50),
            (200, 60),
        ];
        for parts in [DEFAULT.to_vec(), third()] {
            // A state whose body has these parts, for the cursor.
            let mut st = AppState::new();
            st.content.body.retain(|n| parts.contains(&n.part));
            for (part, kind, joins) in [
                (ThirdArm, PartKind::Arm, Torso),
                (ThirdHand, PartKind::Hand, ThirdArm),
            ] {
                if parts.contains(&part) {
                    st.content.body.push(game::BodyNode {
                        part,
                        kind,
                        name: format!("{kind:?} 3").to_uppercase(),
                        joins: Some(joins),
                        main: false,
                    });
                }
            }
            for (w, h) in sizes {
                let l = layout(tab(w, h), &parts);
                for (i, (a, ra)) in l.tiles.iter().enumerate() {
                    assert!(
                        ra.x >= l.left.x
                            && ra.y >= l.left.y
                            && ra.right() <= l.left.right()
                            && ra.bottom() <= l.left.bottom(),
                        "{a:?} {ra:?} outside {:?} at {w}x{h}",
                        l.left
                    );
                    for (b, rb) in &l.tiles[i + 1..] {
                        assert!(!ra.intersects(*rb), "{a:?} on {b:?} at {w}x{h}");
                    }
                }
                for line in &l.lines {
                    for &(x, y) in &line.path {
                        assert!(l.left.contains((x, y).into()), "{line:?} at {w}x{h}");
                    }
                }
                // The arrows reach every tile drawn from TORSO.
                let mut reached = vec![Torso];
                let mut i = 0;
                while let Some(&from) = reached.get(i) {
                    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                        if let Some(to) = step(&l, from, dir)
                            && !reached.contains(&to)
                        {
                            reached.push(to);
                        }
                    }
                    i += 1;
                }
                for (part, _) in &l.tiles {
                    assert!(reached.contains(part), "{part:?} unreached at {w}x{h}");
                }
                // Wherever the cursor was kept, it lands on a tile drawn.
                for &part in &parts {
                    let mut kit = super::super::Kit::new(0);
                    kit.state.part = part;
                    let on = kit.cursor(&st, &l);
                    assert!(l.tile(on).is_some(), "{part:?} → {on:?} at {w}x{h}");
                }
            }
        }
    }

    #[test]
    fn the_arrows_walk_the_body() {
        let l = layout(tab(120, 40), &DEFAULT);
        assert_eq!(step(&l, Torso, Dir::Up), Some(Head));
        assert_eq!(step(&l, Torso, Dir::Left), Some(LeftArm));
        assert_eq!(step(&l, Torso, Dir::Right), Some(RightArm));
        assert_eq!(step(&l, Torso, Dir::Down), Some(Waist));
        assert_eq!(step(&l, RightArm, Dir::Right), Some(RightHand));
        assert_eq!(step(&l, RightHand, Dir::Right), None, "over to the pack");
        assert_eq!(step(&l, Head, Dir::Right), Some(Face));
        assert_eq!(step(&l, Waist, Dir::Left), Some(Back));
        assert_eq!(step(&l, Legs, Dir::Right), Some(Feet));
        assert_eq!(step(&l, Waist, Dir::Down), Some(Legs));
        assert_eq!(step(&l, Head, Dir::Up), None);
        // Compact, the hands sit beside the torso.
        let l = layout(tab(100, 30), &DEFAULT);
        assert_eq!(step(&l, Torso, Dir::Left), Some(LeftHand));
        assert_eq!(step(&l, Torso, Dir::Right), Some(RightHand));
        assert_eq!(step(&l, RightHand, Dir::Right), None);
    }

    #[test]
    fn a_third_arm_lands_in_its_free_places() {
        let l = layout(tab(120, 40), &third());
        let (arm, hand) = (l.tile(ThirdArm).unwrap(), l.tile(ThirdHand).unwrap());
        let (r_arm, r_hand) = (l.tile(RightArm).unwrap(), l.tile(RightHand).unwrap());
        assert_eq!(
            (arm.x, hand.x),
            (r_arm.x, r_hand.x),
            "under R ARM and R HAND"
        );
        assert!(arm.y > r_arm.bottom() && hand.y > r_hand.bottom());
        assert_eq!(step(&l, RightHand, Dir::Down), Some(ThirdHand));
        // Compact, its hand joins the torso, under R HAND.
        let l = layout(tab(100, 30), &third());
        assert!(l.tile(ThirdArm).is_none());
        let (hand, r_hand) = (l.tile(ThirdHand).unwrap(), l.tile(RightHand).unwrap());
        assert_eq!(hand.x, r_hand.x);
        assert!(hand.y > r_hand.bottom());
    }
}
