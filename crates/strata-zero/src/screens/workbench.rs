//! The WORKBENCH tab as a set of drawings, one per recipe, in skill order. A sheet has
//! zone marks round its edge and a field with the build drawn in it; under the rule
//! sit the parts list, one row per input with what the pack holds, and the title
//! block. The field shows the build as a process: the inputs flow into a box with the
//! skill and its steps, and out to the result.

use crate::app::AppState;
use crate::render::glyphs::kind_icon;
use crate::ui::chrome::{BAR_BG, CYAN, DIM, MAGENTA, PANEL_BG, RULE, TEXT, solid};
use crate::ui::hud::cut;
use crossterm::event::KeyCode;
use game::{Content, Recipe};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Zone marks, the header in the rule, the drawing number and the process lines.
const FAINTER: Color = Color::Rgb(0x5c, 0x66, 0x73);
/// Every part on hand.
const GREEN: Color = Color::Rgb(0x5a, 0xc8, 0x8c);
/// Behind the balloon of a part that is short.
const WARN_BG: Color = Color::Rgb(0x2a, 0x06, 0x22);
/// From the rule to below the sheet: the rule, five rows of parts and the bottom edge.
const FOOT: i32 = 7;
/// The title block's text, this far left of the sheet's right side; its rule two more.
const TITLE_W: i32 = 35;
/// Zone marks: a tick on the top edge every this many columns, on the left every
/// `ZONE_ROWS` rows.
const ZONE_COLS: i32 = 12;
const ZONE_ROWS: i32 = 4;
/// An input's box on the process diagram, and where the boxes start in the field.
const BOX_W: i32 = 26;
const BOX_H: i32 = 3;
const BOX_X: i32 = 9;
/// The most rows between two input boxes.
const MAX_GAP: i32 = 3;
/// The narrowest process box.
const PROCESS_W: i32 = 20;
const HINTS: &str = "←→ drawings · ↑↓ parts · space build · tab tabs · esc close";

/// The sheet shown and the part selected on it. The deck holds it and remembers it
/// across closing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Workbench {
    /// Which drawing, in sheet order (`drawings`).
    drawing: usize,
    /// Which input of that drawing's recipe.
    part: usize,
}

impl Workbench {
    /// Packs the sheet into one number for the deck's row memory.
    pub fn pack(&self) -> usize {
        self.drawing << 16 | self.part
    }

    pub fn unpack(v: usize) -> Workbench {
        Workbench {
            drawing: v >> 16,
            part: v & 0xffff,
        }
    }

    /// The nearest sheet that exists: past the last drawing is the last, and a part
    /// past its drawing's inputs is the first.
    pub(crate) fn clamped(&self, c: &Content) -> Workbench {
        let list = drawings(c);
        let Some(last) = list.len().checked_sub(1) else {
            return Workbench::default();
        };
        let drawing = self.drawing.min(last);
        let inputs = c.recipes[list[drawing]].inputs.len();
        Workbench {
            drawing,
            part: if self.part < inputs { self.part } else { 0 },
        }
    }

    /// Draws the sheet over the deck's body.
    pub fn draw(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        let sheet = Rect::new(
            body.x.saturating_add(1),
            body.y,
            body.width.saturating_sub(2),
            body.height,
        )
        .intersection(buf.area);
        if sheet.is_empty() {
            return;
        }
        solid(buf, sheet);
        let l = Layout::of(sheet);
        let mut p = Pen { buf, clip: sheet };
        frame(&mut p, &l);
        let list = drawings(&st.content);
        if list.is_empty() {
            return;
        }
        let at = self.clamped(&st.content);
        let r = &st.content.recipes[list[at.drawing]];
        let ins = inputs(st, r);
        let field = l.field().intersection(sheet);
        process(
            &mut Pen {
                buf: &mut *p.buf,
                clip: field,
            },
            st,
            r,
            &ins,
            at.part,
        );
        parts_list(&mut p, &l, &ins, at.part);
        title_block(&mut p, &l, st, r, &ins, (at.drawing, list.len()));
    }

    /// Takes the sheet's keys; false for any other, which the deck then handles.
    pub fn key(&mut self, st: &mut AppState, code: KeyCode) -> bool {
        let c = &st.content;
        match code {
            KeyCode::Left | KeyCode::Char('h') => self.flip(c, -1),
            KeyCode::Right | KeyCode::Char('l') => self.flip(c, 1),
            KeyCode::Up | KeyCode::Char('k') => self.walk(c, -1),
            KeyCode::Down | KeyCode::Char('j') => self.walk(c, 1),
            _ => return false,
        }
        true
    }

    pub fn hints(&self, _st: &AppState) -> &'static str {
        HINTS
    }

    /// The next or previous drawing, wrapping, on its first part.
    fn flip(&mut self, c: &Content, delta: i32) {
        let n = drawings(c).len() as i32;
        if n == 0 {
            return;
        }
        let at = self.clamped(c).drawing as i32;
        self.drawing = (at + delta).rem_euclid(n) as usize;
        self.part = 0;
    }

    /// The next or previous input on the sheet, wrapping.
    fn walk(&mut self, c: &Content, delta: i32) {
        *self = self.clamped(c);
        let inputs = drawings(c)
            .get(self.drawing)
            .map_or(0, |&i| c.recipes[i].inputs.len()) as i32;
        if inputs > 0 {
            self.part = (self.part as i32 + delta).rem_euclid(inputs) as usize;
        }
    }
}

/// Recipe indices in sheet order: stably by the skill's place in `content.skills`,
/// with any recipe whose skill content doesn't list at the end.
pub(crate) fn drawings(c: &Content) -> Vec<usize> {
    let place = |r: &Recipe| {
        c.skills
            .iter()
            .position(|s| s.id == r.skill)
            .unwrap_or(usize::MAX)
    };
    let mut out: Vec<usize> = (0..c.recipes.len()).collect();
    out.sort_by_key(|&i| place(&c.recipes[i]));
    out
}

/// One input of the recipe on the sheet, with what the pack holds of it.
struct Input {
    icon: char,
    name: String,
    need: u32,
    have: u32,
}

impl Input {
    fn on_hand(&self) -> bool {
        self.have >= self.need
    }
}

fn inputs(st: &AppState, r: &Recipe) -> Vec<Input> {
    r.inputs
        .iter()
        .map(|(id, need)| {
            let item = st.content.item(id);
            Input {
                icon: item.map_or('?', |i| kind_icon(i.kind)),
                name: item.map_or_else(|| id.to_string(), |i| i.name.clone()),
                need: *need,
                have: st.character.count(id),
            }
        })
        .collect()
}

/// Where the sheet's pieces sit, in columns and rows, edges inclusive.
struct Layout {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
    /// The rule over the parts list and the title block.
    rule: i32,
    /// The rule between the parts list and the title block.
    divider: i32,
    /// Where the title block's lines start.
    title: i32,
}

impl Layout {
    fn of(sheet: Rect) -> Layout {
        let (x, y) = (sheet.x as i32, sheet.y as i32);
        let (r, b) = (sheet.right() as i32, sheet.bottom() as i32);
        Layout {
            left: x,
            right: r - 1,
            top: y,
            bottom: b - 1,
            rule: b - FOOT,
            divider: r - TITLE_W - 2,
            title: r - TITLE_W,
        }
    }

    /// Between the top edge and the rule, inside the side edges.
    fn field(&self) -> Rect {
        let w = (self.right - self.left - 1).max(0);
        let h = (self.rule - self.top - 1).max(0);
        if w == 0 || h == 0 {
            return Rect::default();
        }
        Rect::new(
            (self.left + 1) as u16,
            (self.top + 1) as u16,
            w as u16,
            h as u16,
        )
    }

    /// Rows the parts list has between the rule and the bottom edge.
    fn list_rows(&self) -> usize {
        (self.bottom - self.rule - 1).max(0) as usize
    }
}

/// Writes cells clipped to a rectangle, so coordinates may fall outside it. Each
/// write sets the whole cell: no modifier is left from what was there.
struct Pen<'a> {
    buf: &'a mut Buffer,
    clip: Rect,
}

impl Pen<'_> {
    /// `text` from `(x, y)`, stopping before column `max_x`, on the panel unless the
    /// style brings its own background.
    fn put_to(&mut self, x: i32, y: i32, text: &str, style: Style, max_x: i32) {
        let c = self.clip;
        if y < c.y as i32 || y >= c.bottom() as i32 {
            return;
        }
        let style = if style.bg.is_none() {
            style.bg(PANEL_BG)
        } else {
            style
        };
        let end = max_x.min(c.right() as i32);
        for (i, ch) in text.chars().enumerate() {
            let cx = x + i as i32;
            if cx >= end {
                break;
            }
            if cx >= c.x as i32 {
                let cell = &mut self.buf[(cx as u16, y as u16)];
                cell.reset();
                cell.set_char(ch).set_style(style);
            }
        }
    }

    fn put(&mut self, x: i32, y: i32, text: &str, style: Style) {
        self.put_to(x, y, text, style, i32::MAX);
    }

    fn ch(&mut self, x: i32, y: i32, ch: char, style: Style) {
        let mut b = [0; 4];
        self.put(x, y, ch.encode_utf8(&mut b), style);
    }

    /// Lays `bg` under columns `x0..x1` of row `y`.
    fn bar(&mut self, x0: i32, x1: i32, y: i32, bg: Color) {
        let c = self.clip;
        if y < c.y as i32 || y >= c.bottom() as i32 {
            return;
        }
        for x in x0.max(c.x as i32)..x1.min(c.right() as i32) {
            self.buf[(x as u16, y as u16)].set_bg(bg);
        }
    }

    /// A box's outline, edges inclusive, in a set of glyphs: across, down, then the
    /// corners clockwise from the top left.
    fn outline(&mut self, (x0, y0, x1, y1): (i32, i32, i32, i32), set: [char; 6], col: Color) {
        let s = fg(col);
        for x in x0..=x1 {
            self.ch(x, y0, set[0], s);
            self.ch(x, y1, set[0], s);
        }
        for y in y0..=y1 {
            self.ch(x0, y, set[1], s);
            self.ch(x1, y, set[1], s);
        }
        self.ch(x0, y0, set[2], s);
        self.ch(x1, y0, set[3], s);
        self.ch(x1, y1, set[4], s);
        self.ch(x0, y1, set[5], s);
    }
}

const LIGHT: [char; 6] = ['─', '│', '┌', '┐', '┘', '└'];
const DASHED: [char; 6] = ['╌', '╎', '┌', '┐', '┘', '└'];
const DOUBLE: [char; 6] = ['═', '║', '╔', '╗', '╝', '╚'];

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

fn bold(c: Color) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}

fn width(s: &str) -> i32 {
    s.chars().count() as i32
}

/// The sheet's border with its zone marks, the rule with the parts list's header in
/// it, and the divider down to the bottom edge.
fn frame(p: &mut Pen, l: &Layout) {
    let rule = fg(RULE);
    let faint = fg(FAINTER);
    p.outline((l.left, l.top, l.right, l.bottom), LIGHT, RULE);
    let mut k = 1;
    while l.left + ZONE_COLS * k < l.right - 4 {
        let x = l.left + ZONE_COLS * k;
        p.ch(x, l.top, '┬', rule);
        p.put(x - ZONE_COLS / 2, l.top, &k.to_string(), faint);
        k += 1;
    }
    let mut k = 1;
    while l.top + ZONE_ROWS * k < l.rule {
        let y = l.top + ZONE_ROWS * k;
        p.ch(l.left, y, '├', rule);
        let letter = (b'A' + ((k - 1) % 26) as u8) as char;
        p.ch(l.left, y - ZONE_ROWS / 2, letter, faint);
        k += 1;
    }
    for x in l.left + 1..l.right {
        p.ch(x, l.rule, '─', rule);
    }
    p.ch(l.left, l.rule, '├', rule);
    p.ch(l.right, l.rule, '┤', rule);
    p.ch(l.divider, l.rule, '┬', rule);
    for y in l.rule + 1..l.bottom {
        p.ch(l.divider, y, '│', rule);
    }
    p.ch(l.divider, l.bottom, '┴', rule);
    // The parts list's header rides the rule.
    let x0 = l.left + 1;
    for (dx, t) in [(2, " # "), (5, " PART "), (28, " QTY "), (34, " ON HAND ")] {
        p.put(x0 + dx, l.rule, t, faint);
    }
}

/// The first of `n` rows to show in `rows` so that `sel` is in view, near the middle.
fn scroll(sel: usize, n: usize, rows: usize) -> usize {
    let half = rows.saturating_sub(1) / 2;
    sel.saturating_sub(half).min(n.saturating_sub(rows))
}

/// One row per input under the rule: the cursor, the number, the name, how many the
/// build takes and how many the pack holds, and whether that is enough.
fn parts_list(p: &mut Pen, l: &Layout, ins: &[Input], sel: usize) {
    let x0 = l.left + 1;
    let rows = l.list_rows();
    let first = scroll(sel, ins.len(), rows);
    for (i, input) in ins.iter().enumerate().skip(first).take(rows) {
        let y = l.rule + 1 + (i - first) as i32;
        let lit = i == sel;
        let ok = input.on_hand();
        let bg = if lit { BAR_BG } else { PANEL_BG };
        if lit {
            p.bar(x0, l.divider, y, BAR_BG);
        }
        let col = if lit {
            CYAN
        } else if ok {
            TEXT
        } else {
            MAGENTA
        };
        let plain = fg(col).bg(bg);
        let strong = if lit {
            plain.add_modifier(Modifier::BOLD)
        } else {
            plain
        };
        if lit {
            p.put(x0 + 1, y, "▸", plain);
        }
        p.put(x0 + 3, y, &(i + 1).to_string(), strong);
        p.put_to(x0 + 6, y, &cut(&input.name, 23), strong, x0 + 29);
        p.put_to(x0 + 30, y, &input.need.to_string(), plain, x0 + 35);
        p.put_to(x0 + 36, y, &input.have.to_string(), plain, x0 + 41);
        let mark = if ok { ("■", CYAN) } else { ("□", MAGENTA) };
        p.put(x0 + 42, y, mark.0, fg(mark.1).bg(bg));
        if !ok {
            p.put_to(x0 + 45, y, "not in pack", fg(MAGENTA).bg(bg), l.divider);
        }
    }
}

/// The title block: the recipe and where it sits in the set, what it makes, the skill
/// it needs, how much of it is on hand, and the drawing's number.
fn title_block(
    p: &mut Pen,
    l: &Layout,
    st: &AppState,
    r: &Recipe,
    ins: &[Input],
    (i, n): (usize, usize),
) {
    let x = l.title;
    let y = l.rule + 1;
    let edge = l.right;
    let index = format!("◂ {}/{n} ▸", i + 1);
    let ix = l.right - 3 - width(&index);
    let room = (ix - 1 - x).max(0) as usize;
    p.put_to(x, y, &cut(&r.name.to_uppercase(), room), bold(CYAN), ix - 1);
    p.put_to(ix, y, &index, fg(DIM), edge);

    let (out, count) = (&r.output.0, r.output.1);
    let makes = match st.content.item(out) {
        Some(item) => format!("makes {count}× {} · {}", item.name, item.kind.label()),
        None => format!("makes {count}× {out}"),
    };
    p.put_to(x, y + 1, &makes, fg(DIM), edge);

    let skill = skill_name(st, r);
    let needs = if st.character.has_skill(&r.skill) {
        (format!("needs {skill} · yours"), DIM)
    } else {
        (format!("needs {skill}"), MAGENTA)
    };
    p.put_to(x, y + 2, &needs.0, fg(needs.1), edge);

    let short: Vec<&Input> = ins.iter().filter(|i| !i.on_hand()).collect();
    let hand = match short.as_slice() {
        [] => ("every part on hand".to_string(), GREEN),
        [one] => (
            format!("{} more {}", one.need - one.have, one.name),
            MAGENTA,
        ),
        _ => (
            format!("{} of {} parts on hand", ins.len() - short.len(), ins.len()),
            MAGENTA,
        ),
    };
    p.put_to(x, y + 3, &hand.0, fg(hand.1), edge);

    let dwg = format!(
        "DWG {}/{n} · process · drawn {}",
        i + 1,
        st.character.handle.to_uppercase()
    );
    p.put_to(x, y + 4, &dwg, fg(FAINTER), edge);
}

/// The recipe's skill by name, or its id when content doesn't know it.
fn skill_name(st: &AppState, r: &Recipe) -> String {
    st.content
        .skill(&r.skill)
        .map_or_else(|| r.skill.to_string(), |s| s.name.clone())
}

/// A part's number in a balloon: cyan on the bar's colour when on hand, magenta on
/// a dark magenta when short; the selected one's inverted.
fn balloon(p: &mut Pen, x: i32, y: i32, n: usize, ok: bool, sel: bool) {
    let (ink, under) = if ok {
        (CYAN, BAR_BG)
    } else {
        (MAGENTA, WARN_BG)
    };
    let style = if sel {
        bold(PANEL_BG).bg(ink)
    } else {
        bold(ink).bg(under)
    };
    p.put(x, y, &format!(" {n} "), style);
}

/// The glyph where the junction's lines meet in a cell.
fn junction(up: bool, down: bool, left: bool, right: bool) -> char {
    match (up, down, left, right) {
        (true, true, true, true) => '┼',
        (true, true, true, false) => '┤',
        (true, true, false, true) => '├',
        (true, true, false, false) => '│',
        (false, true, true, true) => '┬',
        (true, false, true, true) => '┴',
        (false, true, true, false) => '┐',
        (true, false, true, false) => '┘',
        (false, true, false, true) => '┌',
        (true, false, false, true) => '└',
        _ => '─',
    }
}

/// The process diagram in the field: the inputs stacked down the left, lines from
/// them meeting at a junction and running into the process (the skill on its top
/// edge, the steps inside), and on out to the result.
fn process(p: &mut Pen, st: &AppState, r: &Recipe, ins: &[Input], sel: usize) {
    let f = p.clip;
    if f.is_empty() {
        return;
    }
    let (fx, fy, fh) = (f.x as i32, f.y as i32, f.height as i32);
    let bx = fx + BOX_X;
    let jx = bx + BOX_W + 6;
    // The diagram hangs a row above the field's middle, where it reads as centred.
    let centre = (fy + (fh - 1) / 2 - 1).max(fy);
    // The row each shown input's line leaves from.
    let mut lines: Vec<i32> = Vec::new();
    match stack(ins.len(), centre, fy, fy + fh) {
        Some((top, gap)) => {
            for (i, input) in ins.iter().enumerate() {
                let y = top + i as i32 * (BOX_H + gap);
                input_box(p, bx, y, i + 1, input, i == sel);
                lines.push(y + 1);
            }
        }
        None => {
            // Too many for boxes: one line each, scrolled to keep the selected in view.
            let rows = (fh as usize).min(ins.len());
            let first = scroll(sel, ins.len(), rows);
            let top = (centre - (rows as i32 - 1) / 2).clamp(fy, fy + fh - rows as i32);
            for (k, (i, input)) in ins.iter().enumerate().skip(first).take(rows).enumerate() {
                let y = top + k as i32;
                input_line(p, bx, y, i + 1, input, i == sel);
                lines.push(y);
            }
        }
    }
    let faint = fg(FAINTER);
    let mid = match (lines.first(), lines.last()) {
        (Some(&a), Some(&b)) => (a + b) / 2,
        _ => centre,
    };
    for &y in &lines {
        for x in bx + BOX_W..jx {
            p.ch(x, y, '─', faint);
        }
    }
    if let (Some(&top), Some(&bot)) = (lines.first(), lines.last()) {
        for y in top..=bot {
            let glyph = junction(y > top, y < bot, lines.contains(&y), y == mid);
            p.ch(jx, y, glyph, faint);
        }
        p.put(jx + 1, mid, "────▶", faint);
    }

    // The process: the skill set into its top edge, the steps numbered inside.
    let skill = skill_name(st, r).to_uppercase();
    let steps: Vec<String> = r
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{} {s}", i + 1))
        .collect();
    let longest = steps.iter().map(|s| width(s)).max().unwrap_or(0);
    let w = PROCESS_W.max(longest + 6).max(width(&skill) + 4);
    let (px0, px1) = (jx + 7, jx + 6 + w);
    p.outline((px0, mid - 3, px1, mid + 3), DOUBLE, DIM);
    p.put(
        px0 + (px1 - px0 - width(&skill)) / 2 + 1,
        mid - 3,
        &skill,
        bold(CYAN),
    );
    for (i, s) in steps.iter().enumerate() {
        p.put_to(px0 + 3, mid - 1 + i as i32, s, fg(TEXT), px1);
    }

    // Out to the result.
    p.put(px1 + 1, mid, "────▶", faint);
    let ox0 = px1 + 7;
    let ox1 = f.right() as i32 - 2;
    if ox1 <= ox0 + 2 {
        return;
    }
    p.outline((ox0, mid - 1, ox1, mid + 1), LIGHT, CYAN);
    let (out, count) = (&r.output.0, r.output.1);
    let item = st.content.item(out);
    let icon = item.map_or('?', |i| kind_icon(i.kind));
    let name = item.map_or_else(|| out.to_string(), |i| i.name.clone());
    p.ch(ox0 + 2, mid, icon, bold(CYAN));
    let room = (ox1 - 1 - (ox0 + 4)).max(0) as usize;
    p.put_to(
        ox0 + 4,
        mid,
        &cut(&format!("{count}× {name}"), room),
        bold(CYAN),
        ox1,
    );
}

/// Where `n` input boxes go, stacked round `centre` between rows `y0..y1`: the top
/// box's row and the rows between boxes, as many as fit up to `MAX_GAP`. `None`
/// when even touching they don't fit.
fn stack(n: usize, centre: i32, y0: i32, y1: i32) -> Option<(i32, i32)> {
    if n == 0 {
        return Some((centre, 0));
    }
    let n = n as i32;
    let fits = |top: i32, span: i32| top >= y0 && top + span <= y1;
    for gap in (0..=MAX_GAP).rev() {
        let span = n * BOX_H + (n - 1) * gap;
        let top = centre - (span - 1) / 2;
        if fits(top, span) {
            return Some((top, gap));
        }
    }
    let span = n * BOX_H;
    (span <= y1 - y0).then(|| (y0 + (y1 - y0 - span) / 2, 0))
}

/// An input as a box: a thin outline when on hand, a dashed magenta one when short,
/// cyan when selected; the kind's icon, the name and how many, its balloon to the left.
fn input_box(p: &mut Pen, x: i32, y: i32, n: usize, input: &Input, sel: bool) {
    let ok = input.on_hand();
    let edge = if sel {
        CYAN
    } else if ok {
        RULE
    } else {
        MAGENTA
    };
    let set = if ok { LIGHT } else { DASHED };
    p.outline((x, y, x + BOX_W - 1, y + BOX_H - 1), set, edge);
    input_text(p, x, y + 1, input, None);
    balloon(p, x - 4, y + 1, n, ok, sel);
}

/// An input on one line, for when they don't fit as boxes: the box's middle row
/// without the box, the selected one in cyan.
fn input_line(p: &mut Pen, x: i32, y: i32, n: usize, input: &Input, sel: bool) {
    input_text(p, x, y, input, sel.then_some(CYAN));
    balloon(p, x - 4, y, n, input.on_hand(), sel);
}

/// The icon, the name and `×n` inside an input's box, all in `tone` when given.
fn input_text(p: &mut Pen, x: i32, y: i32, input: &Input, tone: Option<Color>) {
    let (mark, text) = match tone {
        Some(c) => (c, c),
        None if input.on_hand() => (DIM, TEXT),
        None => (MAGENTA, MAGENTA),
    };
    let count = format!("×{}", input.need);
    let cx = x + BOX_W - 2 - width(&count);
    p.ch(x + 2, y, input.icon, bold(mark));
    let room = (cx - 1 - (x + 4)).max(0) as usize;
    let name = if tone.is_some() { bold(text) } else { fg(text) };
    p.put_to(x + 4, y, &cut(&input.name, room), name, cx - 1);
    p.put(cx, y, &count, fg(mark));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::deck::{Deck, Tab};
    use crate::screens::testutil::key;
    use crate::ui::chrome::{CYAN, MAGENTA};
    use crossterm::event::KeyCode;
    use game::{Character, Content, ItemId, RecipeId, SkillId};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    /// D'SHAWN, the debug runner, who has the bench and most of the skills.
    fn dshawn() -> AppState {
        let mut st = AppState::new();
        st.character = Character::debug_runner(&st.content);
        // Clear of the deck's signal jitter, which glitches a row on its first ticks.
        st.tick = 40;
        st
    }

    /// The deck opened on the WORKBENCH tab.
    fn bench(st: &mut AppState) -> Deck {
        let mut d = Deck::new(0);
        d.update(st, &[key(KeyCode::Char('4'))]);
        assert_eq!(d.tab, Tab::Workbench);
        d
    }

    fn press(d: &mut Deck, st: &mut AppState, code: KeyCode, times: usize) {
        for _ in 0..times {
            d.update(st, &[key(code)]);
        }
    }

    fn render(d: &Deck, st: &AppState, w: u16, h: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    /// `len` cells of row `y` from column `x`.
    fn cells(b: &Buffer, x: u16, y: u16, len: usize) -> String {
        (x..x + len as u16).map(|cx| b[(cx, y)].symbol()).collect()
    }

    fn row(b: &Buffer, y: u16) -> String {
        cells(b, 0, y, b.area.width as usize)
    }

    /// Rows `y0..y1` between columns `x0..x1`, joined by newlines.
    fn area(b: &Buffer, x0: u16, x1: u16, y0: u16, y1: u16) -> String {
        (y0..y1)
            .map(|y| cells(b, x0, y, (x1 - x0) as usize))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn recipe_id(st: &AppState, d: &Deck) -> String {
        let list = drawings(&st.content);
        st.content.recipes[list[d.workbench.drawing]].id.to_string()
    }

    #[test]
    fn drawings_go_by_skill() {
        let c = Content::load().unwrap();
        let ids: Vec<String> = drawings(&c)
            .iter()
            .map(|&i| c.recipes[i].id.to_string())
            .collect();
        assert_eq!(
            ids,
            [
                "wire_shiv",
                "hardhat_visor",
                "optics_tune",
                "servo_rebuild",
                "caps_fence",
                "stim_batch",
                "ratchet_build"
            ]
        );
    }

    #[test]
    fn the_sheet_frames_the_body() {
        let mut st = dshawn();
        let d = bench(&mut st);
        let b = render(&d, &st, 100, 30);
        assert_eq!(b[(2, 5)].symbol(), "┌", "{}", row(&b, 5));
        assert_eq!(b[(97, 5)].symbol(), "┐", "{}", row(&b, 5));
        assert_eq!(b[(2, 26)].symbol(), "└", "{}", row(&b, 26));
        assert_eq!(b[(61, 26)].symbol(), "┴", "{}", row(&b, 26));
        assert_eq!(b[(97, 26)].symbol(), "┘", "{}", row(&b, 26));
        let rule = row(&b, 20);
        assert_eq!(b[(2, 20)].symbol(), "├", "{rule}");
        assert_eq!(cells(&b, 5, 20, 3), " # ", "{rule}");
        assert_eq!(cells(&b, 8, 20, 6), " PART ", "{rule}");
        assert_eq!(cells(&b, 31, 20, 5), " QTY ", "{rule}");
        assert_eq!(cells(&b, 37, 20, 9), " ON HAND ", "{rule}");
        assert_eq!(b[(61, 20)].symbol(), "┬", "{rule}");
        assert_eq!(b[(97, 20)].symbol(), "┤", "{rule}");
        // Zone marks: letters down the left edge, numbers along the top.
        assert_eq!(b[(2, 7)].symbol(), "A");
        assert_eq!(b[(8, 5)].symbol(), "1");
        assert_eq!(b[(14, 5)].symbol(), "┬");
        assert_eq!(b[(2, 9)].symbol(), "├");

        // Drawn alone over a probe, the sheet writes nothing outside the body: row 27
        // is the status strip's.
        let body = Rect::new(1, 5, 98, 22);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        for y in 0..30 {
            for x in 0..100 {
                buf[(x, y)].set_char('Ж');
            }
        }
        d.workbench.draw(&st, body, &mut buf);
        for y in 0..30u16 {
            for x in 0..100u16 {
                let inside = x >= body.x && x < body.right() && y >= body.y && y < body.bottom();
                if !inside {
                    assert_eq!(buf[(x, y)].symbol(), "Ж", "({x}, {y}) written");
                }
            }
        }
    }

    #[test]
    fn a_process_sheet() {
        let mut st = dshawn();
        st.character.give(ItemId::new("copper_coil"), 1);
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Right, 5);
        assert_eq!(recipe_id(&st, &d), "stim_batch");
        let b = render(&d, &st, 100, 30);
        let title = area(&b, 62, 97, 21, 26);
        assert_eq!(cells(&b, 63, 21, 10), "STIM BATCH", "{title}");
        assert_eq!(cells(&b, 87, 21, 7), "◂ 6/7 ▸", "{title}");
        assert_eq!(
            cells(&b, 63, 22, 32),
            "makes 2× Stim patch · consumable",
            "{title}"
        );
        assert_eq!(cells(&b, 63, 23, 21).trim_end(), "needs Medtech", "{title}");
        assert_eq!(b[(63, 23)].fg, MAGENTA);
        assert_eq!(cells(&b, 63, 24, 18), "every part on hand", "{title}");
        assert_eq!(b[(63, 24)].fg, Color::Rgb(0x5a, 0xc8, 0x8c));
        assert_eq!(
            cells(&b, 63, 25, 33),
            "DWG 6/7 · process · drawn D'SHAWN",
            "{title}"
        );

        let parts = area(&b, 3, 61, 21, 26);
        assert_eq!(cells(&b, 6, 21, 14), "1  Copper coil", "{parts}");
        assert_eq!(b[(33, 21)].symbol(), "1", "{parts}");
        let coils = st.character.count(&ItemId::new("copper_coil")).to_string();
        assert_eq!(cells(&b, 39, 21, coils.len()), coils, "{parts}");
        assert_eq!(b[(45, 21)].symbol(), "■", "{parts}");
        assert_eq!(b[(45, 21)].fg, CYAN);
        assert_eq!(cells(&b, 6, 22, 17), "2  Soy-noodle cup", "{parts}");
        // The selected row has the bar and the cursor.
        assert_eq!(b[(4, 21)].symbol(), "▸");
        assert_eq!(b[(3, 21)].bg, crate::ui::chrome::BAR_BG);
        assert_eq!(b[(60, 21)].bg, crate::ui::chrome::BAR_BG);

        let field = area(&b, 3, 97, 6, 20);
        for want in ["MEDTECH", "1 boil", "2 filter", "3 pray", "2× Stim patch"] {
            assert!(field.contains(want), "{want}:\n{field}");
        }
        // The two inputs as boxes, their balloons to the left, into the process.
        assert_eq!(b[(12, 7)].symbol(), "┌", "{field}");
        assert_eq!(b[(37, 9)].symbol(), "┘", "{field}");
        assert_eq!(cells(&b, 8, 8, 3), " 1 ", "{field}");
        assert_eq!(b[(12, 13)].symbol(), "┌", "{field}");
        assert_eq!(b[(44, 8)].symbol(), "┐", "{field}");
        assert_eq!(b[(44, 14)].symbol(), "┘", "{field}");
        assert_eq!(cells(&b, 44, 11, 6), "├────▶", "{field}");
        assert_eq!(b[(51, 8)].symbol(), "╔", "{field}");
        assert_eq!(b[(70, 14)].symbol(), "╝", "{field}");
        assert_eq!(cells(&b, 58, 8, 7), "MEDTECH", "{field}");
        assert_eq!(cells(&b, 54, 10, 6), "1 boil", "{field}");
        assert_eq!(cells(&b, 71, 11, 5), "────▶", "{field}");
        assert_eq!(b[(77, 10)].symbol(), "┌", "{field}");
        assert_eq!(b[(95, 12)].symbol(), "┘", "{field}");
        assert_eq!(cells(&b, 81, 11, 13), "2× Stim patch", "{field}");
    }

    #[test]
    fn a_short_input_is_dashed_and_named() {
        let mut st = dshawn();
        st.character.give(ItemId::new("copper_coil"), 2);
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Right, 3);
        assert_eq!(recipe_id(&st, &d), "servo_rebuild");
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        // The servos, short, in a dashed box; the coils, on hand, in a solid one.
        assert_eq!(cells(&b, 13, 7, 3), "╌╌╌", "{field}");
        assert_eq!(b[(12, 8)].symbol(), "╎", "{field}");
        assert_eq!(cells(&b, 13, 13, 3), "───", "{field}");
        assert_eq!(cells(&b, 63, 21, 13), "SERVO REBUILD");
        assert_eq!(cells(&b, 48, 21, 11), "not in pack", "{}", row(&b, 21));
        assert_eq!(b[(48, 21)].fg, MAGENTA);
        assert_eq!(b[(45, 21)].symbol(), "□");
        assert_ne!(cells(&b, 48, 22, 11), "not in pack", "{}", row(&b, 22));
        assert_eq!(
            cells(&b, 63, 24, 18),
            "2 more Scrap servo",
            "{}",
            row(&b, 24)
        );
        assert_eq!(b[(63, 24)].fg, MAGENTA);
        assert_eq!(
            cells(&b, 63, 23, 25),
            "needs Electronics · yours",
            "{}",
            row(&b, 23)
        );
    }

    #[test]
    fn keys_flip_and_walk() {
        let mut st = dshawn();
        let mut d = bench(&mut st);
        assert_eq!(recipe_id(&st, &d), "wire_shiv");
        press(&mut d, &mut st, KeyCode::Right, 7);
        assert_eq!(recipe_id(&st, &d), "wire_shiv", "→ wraps through all seven");
        press(&mut d, &mut st, KeyCode::Left, 1);
        assert_eq!(recipe_id(&st, &d), "ratchet_build", "← wraps to the last");
        let b = render(&d, &st, 100, 30);
        assert_eq!(cells(&b, 63, 21, 11), "RATCHET SMG");
        assert_eq!(cells(&b, 87, 21, 7), "◂ 7/7 ▸");
        press(&mut d, &mut st, KeyCode::Left, 6);
        assert_eq!(recipe_id(&st, &d), "wire_shiv");

        // Two inputs: ↓ and ↑ wrap through them.
        press(&mut d, &mut st, KeyCode::Down, 1);
        assert_eq!(d.workbench.part, 1);
        press(&mut d, &mut st, KeyCode::Down, 1);
        assert_eq!(d.workbench.part, 0);
        press(&mut d, &mut st, KeyCode::Up, 1);
        assert_eq!(d.workbench.part, 1);
        let b = render(&d, &st, 100, 30);
        assert_eq!(b[(4, 22)].symbol(), "▸", "{}", row(&b, 22));
        assert_eq!(b[(4, 21)].symbol(), " ", "{}", row(&b, 21));
        // Flipping starts the next sheet on its first part.
        press(&mut d, &mut st, KeyCode::Right, 1);
        assert_eq!(d.workbench.part, 0);
        assert_eq!(recipe_id(&st, &d), "hardhat_visor");

        let b = render(&d, &st, 100, 30);
        assert_eq!(
            cells(&b, 2, 28, 60).trim_end(),
            "←→ drawings · ↑↓ parts · space build · tab tabs · esc close"
        );
        // Tab still goes to the next tab; esc still closes.
        press(&mut d, &mut st, KeyCode::Tab, 1);
        assert_eq!(d.tab, Tab::Jobs);
        press(&mut d, &mut st, KeyCode::BackTab, 1);
        assert_eq!(d.tab, Tab::Workbench);
        assert_eq!(recipe_id(&st, &d), "hardhat_visor", "the sheet stays put");
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn the_sheet_reopens_where_it_was() {
        let mut st = dshawn();
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Right, 3);
        press(&mut d, &mut st, KeyCode::Down, 1);
        let was = d.workbench.clone();
        assert_eq!((was.drawing, was.part), (3, 1));
        d.update(&mut st, &[key(KeyCode::Esc)]);
        let again = Deck::new(5).restore(&st);
        assert_eq!(again.tab, Tab::Workbench);
        assert_eq!(again.workbench, was);

        let back = Workbench::unpack(was.pack());
        assert_eq!(back, was);
        // Past the counts: the last drawing, on its first part.
        st.deck_rows[Tab::Workbench.index()] = Workbench {
            drawing: 40,
            part: 9,
        }
        .pack();
        let clamped = Deck::new(6).restore(&st);
        assert_eq!(
            clamped.workbench,
            Workbench {
                drawing: 6,
                part: 0
            }
        );
    }

    #[test]
    fn many_inputs_scroll() {
        let mut st = dshawn();
        let mut r = st.content.recipes[0].clone();
        r.id = RecipeId::new("seven_up");
        r.name = "Seven up".into();
        r.skill = SkillId::new("scrapcraft");
        r.inputs = [
            "copper_coil",
            "scrap_servo",
            "dead_phone",
            "soy_noodle_cup",
            "bottle_caps",
            "stim_patch",
            "ratchet_mag",
        ]
        .iter()
        .map(|id| (ItemId::new(id), 1))
        .collect();
        st.content.recipes.push(r);
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Left, 1);
        assert_eq!(recipe_id(&st, &d), "seven_up");
        press(&mut d, &mut st, KeyCode::Up, 1);
        assert_eq!(d.workbench.part, 6, "↑ wraps to the last input");
        let b = render(&d, &st, 100, 30);
        let parts = area(&b, 3, 61, 21, 26);
        // Five rows, scrolled so the seventh input's row is the last one shown.
        let numbers: Vec<String> = (21..26).map(|y| cells(&b, 6, y, 1)).collect();
        assert_eq!(numbers, ["3", "4", "5", "6", "7"], "{parts}");
        assert_eq!(b[(4, 25)].symbol(), "▸", "{parts}");
        assert_eq!(b[(2, 26)].symbol(), "└", "the bottom edge stays");
        // The field lists the inputs one to a line: no boxes, every name shown.
        let field = area(&b, 3, 97, 6, 20);
        assert!(!area(&b, 12, 38, 6, 20).contains('┌'), "{field}");
        for name in ["Copper coil", "Scrap servo", "Stim patch", "Magazine"] {
            assert!(field.contains(name), "{name}:\n{field}");
        }
        assert!(field.contains(" 7 "), "{field}");
        // Every way of walking it draws.
        for _ in 0..8 {
            press(&mut d, &mut st, KeyCode::Down, 1);
            render(&d, &st, 100, 30);
        }
    }

    #[test]
    fn odd_outputs_still_draw() {
        let mut st = dshawn();
        let mut r = st.content.recipes[0].clone();
        r.id = RecipeId::new("mystery");
        r.name = "Mystery".into();
        r.skill = SkillId::new("no_such_skill");
        r.output = (ItemId::new("oddity"), 3);
        r.inputs.push((ItemId::new("no_such_part"), 2));
        r.steps = Vec::new();
        st.content.recipes.push(r);
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Left, 1);
        assert_eq!(recipe_id(&st, &d), "mystery");
        let b = render(&d, &st, 100, 30);
        let title = area(&b, 62, 97, 21, 26);
        assert_eq!(
            cells(&b, 63, 22, 20).trim_end(),
            "makes 3× oddity",
            "{title}"
        );
        assert!(title.contains("DWG 8/8 · process"), "{title}");
        let field = area(&b, 3, 97, 6, 20);
        assert!(field.contains("no_such_part"), "{field}");
        assert!(field.contains("? 3× oddity"), "{field}");
        assert!(field.contains("NO_SUCH_SKILL"), "{field}");
    }

    #[test]
    fn big_terminal_and_long_handle() {
        let mut st = dshawn();
        st.character.handle = "ABCDEFGHIJKLMNOP".into();
        let d = bench(&mut st);
        let dwg = "DWG 1/7 · process · drawn ABCDEFGHIJKLMNOP";

        let b = render(&d, &st, 100, 30);
        assert_eq!(b[(97, 25)].symbol(), "│", "{}", row(&b, 25));
        let shown: String = dwg.chars().take(34).collect();
        assert_eq!(cells(&b, 63, 25, 34), shown, "{}", row(&b, 25));

        let b = render(&d, &st, 120, 40);
        let top = (0..40u16)
            .find(|&y| b[(2, y)].symbol() == "┌")
            .expect("the sheet's top edge");
        assert_eq!(b[(117, top)].symbol(), "┐", "{}", row(&b, top));
        let rule = (top..40)
            .find(|&y| b[(117, y)].symbol() == "┤")
            .expect("the rule");
        assert_eq!(b[(117, rule + 6)].symbol(), "┘", "the foot keeps its rows");
        assert_eq!(
            cells(&b, 83, rule + 1, 9),
            "WIRE SHIV",
            "{}",
            row(&b, rule + 1)
        );
        assert_eq!(b[(81, rule + 1)].symbol(), "│", "{}", row(&b, rule + 1));
        let shown: String = dwg.chars().take(34).collect();
        assert_eq!(cells(&b, 83, rule + 5, 34), shown, "{}", row(&b, rule + 5));
        assert_eq!(b[(117, rule + 5)].symbol(), "│", "{}", row(&b, rule + 5));
    }

    #[test]
    fn every_sheet_is_single_width_and_rgb() {
        use unicode_width::UnicodeWidthStr;
        let mut st = dshawn();
        let mut d = bench(&mut st);
        for _ in 0..drawings(&st.content).len() {
            for _ in 0..3 {
                let b = render(&d, &st, 100, 30);
                for y in 5..27u16 {
                    for x in 2..98u16 {
                        let c = &b[(x, y)];
                        assert_eq!(c.symbol().width(), 1, "({x}, {y}) {:?}", c.symbol());
                        for col in [c.fg, c.bg] {
                            assert!(
                                matches!(col, Color::Rgb(..) | Color::Reset),
                                "({x}, {y}) {col:?}"
                            );
                        }
                    }
                }
                press(&mut d, &mut st, KeyCode::Down, 1);
            }
            press(&mut d, &mut st, KeyCode::Right, 1);
        }
    }
}
