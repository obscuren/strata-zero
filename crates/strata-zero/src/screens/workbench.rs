//! The WORKBENCH tab as a set of drawings, one per recipe, in skill order. A sheet has
//! zone marks round its edge and a field with the build drawn in it; under the rule
//! sit the parts list, one row per input with what the pack holds, and the title
//! block. When the output's drawing declares its parts, the field shows it exploded:
//! the parts drawn apart with their numbers in balloons, a part short as a ghost.
//! Otherwise it shows the build as a process: the inputs flow into a box with the
//! skill and its steps, and out to the result.

use crate::actions::{perform, report};
use crate::anim::Easing;
use crate::app::AppState;
use crate::render::glyphs::kind_icon;
use crate::ui::art::{Art, Dir, PartPlace, brackets, grey_at};
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, MAGENTA, PANEL_BG, RULE, TEXT, solid};
use crate::ui::hud::cut;
use crate::ui::looks::ItemArt;
use crossterm::event::KeyCode;
use game::{Action, Content, ItemId, Recipe};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

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
/// The field the process diagram is laid out for, 100×30's. A wider one keeps the
/// diagram this size and centres it.
const PROCESS_FIELD: i32 = 94;
/// A part short, greyed as the modding mode greys but darker: its luminance times this.
const GHOST: f64 = 0.26;
/// The glyphs a ghost keeps; any other is text, and blanked.
const PIXELS: [&str; 6] = [" ", "▀", "▄", "█", "▌", "▐"];
/// How far the selected part's brackets stand off its rectangle: columns, then rows,
/// before the corners' own cell.
const BRACKET_ROOM: (i32, i32) = (2, 1);
/// How long the parts take to close, then how long the finished build is held, in ticks.
const CLOSE_TICKS: u64 = 15;
const HOLD_TICKS: u64 = 30;
const HINTS: &str = "←→ drawings · ↑↓ parts · space build · q/e tabs · tab close";
const HINTS_ASSEMBLY: &str = "←→ drawings · ↑↓ parts · space assemble · q/e tabs · tab close";

/// The sheet shown and the part selected on it. The deck holds it and remembers it
/// across closing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Workbench {
    /// Which drawing, in sheet order (`drawings`).
    drawing: usize,
    /// Which input of that drawing's recipe.
    part: usize,
    /// The build being shown closing. It is not remembered across closing.
    anim: Option<Built>,
}

/// A build the sheet is showing: the tick it started, the drawing it was on, and what
/// the pack held of each input before it, which the sheet reads until it is over.
#[derive(Clone, Debug, PartialEq)]
struct Built {
    start: u64,
    drawing: usize,
    had: Vec<u32>,
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
            anim: None,
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
            anim: self.anim.clone(),
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
        let mut ins = inputs(st, r);
        let run = self.running(st);
        // While the build runs the whole sheet reads the pack as it was before it.
        if let Some(b) = run {
            for (input, &had) in ins.iter_mut().zip(&b.had) {
                input.have = had;
            }
        }
        let art = assembly(st, r);
        let t = run.map(|b| st.tick.saturating_sub(b.start));
        let mut field = Pen {
            buf: &mut *p.buf,
            clip: l.field().intersection(sheet),
        };
        match &art {
            Some(art) => exploded(&mut field, st, art, &ins, (at.part, t)),
            None => process(&mut field, st, r, &ins, (at.part, t)),
        }
        parts_list(&mut p, &l, &ins, at.part);
        let sheet_no = (at.drawing, list.len());
        title_block(&mut p, &l, st, r, &ins, sheet_no, art.is_some());
    }

    /// Takes the sheet's keys; false for any other, which the deck then handles.
    pub fn key(&mut self, st: &mut AppState, code: KeyCode) -> bool {
        // A build closing takes any key, which only ends it.
        let running = self.running(st).is_some();
        self.anim = None;
        if running {
            return true;
        }
        let c = &st.content;
        match code {
            KeyCode::Char(' ') | KeyCode::Enter => self.build(st),
            KeyCode::Left | KeyCode::Char('h') => self.flip(c, -1),
            KeyCode::Right | KeyCode::Char('l') => self.flip(c, 1),
            KeyCode::Up | KeyCode::Char('k') => self.walk(c, -1),
            KeyCode::Down | KeyCode::Char('j') => self.walk(c, 1),
            _ => return false,
        }
        true
    }

    /// Space assembles on an exploded sheet and builds on a process one.
    pub fn hints(&self, st: &AppState) -> &'static str {
        let c = &st.content;
        let shown = drawings(c).get(self.clamped(c).drawing).copied();
        match shown {
            Some(i) if assembly(st, &c.recipes[i]).is_some() => HINTS_ASSEMBLY,
            _ => HINTS,
        }
    }

    /// The build on the sheet shown, while it is still closing or held.
    fn running(&self, st: &AppState) -> Option<&Built> {
        let shown = self.clamped(&st.content).drawing;
        self.anim.as_ref().filter(|b| {
            b.drawing == shown && st.tick.saturating_sub(b.start) < CLOSE_TICKS + HOLD_TICKS
        })
    }

    /// Builds the recipe on the sheet, logging the result, and starts the animation
    /// when it was made.
    fn build(&mut self, st: &mut AppState) {
        let at = self.clamped(&st.content).drawing;
        let Some(&i) = drawings(&st.content).get(at) else {
            return;
        };
        let r = &st.content.recipes[i];
        let id = r.id.clone();
        let had = r
            .inputs
            .iter()
            .map(|(id, _)| st.character.count(id))
            .collect();
        let result = perform(st, Action::Build(id));
        if result.is_ok() {
            self.anim = Some(Built {
                start: st.tick,
                drawing: at,
                had,
            });
        }
        report(st, result);
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
    id: ItemId,
    icon: char,
    name: String,
    need: u32,
    have: u32,
}

impl Input {
    fn on_hand(&self) -> bool {
        self.have >= self.need
    }

    /// How an input short reads: `none` when the pack holds none of it, else how many
    /// more the build takes.
    fn shortfall(&self, none: &str) -> String {
        if self.have == 0 {
            none.to_string()
        } else {
            format!("{} short", self.need.saturating_sub(self.have))
        }
    }
}

fn inputs(st: &AppState, r: &Recipe) -> Vec<Input> {
    r.inputs
        .iter()
        .map(|(id, need)| {
            let item = st.content.item(id);
            Input {
                id: id.clone(),
                icon: item.map_or('?', |i| kind_icon(i.kind)),
                name: item.map_or_else(|| id.to_string(), |i| i.name.clone()),
                need: *need,
                have: st.character.count(id),
            }
        })
        .collect()
}

/// The output's drawing when it declares the parts it is built from, with nothing
/// fitted and no stickers, as a build makes it; none otherwise.
fn assembly(st: &AppState, r: &Recipe) -> Option<ItemArt> {
    let item = st.content.item(&r.output.0)?;
    let art = ItemArt {
        look: item.look?,
        fitted: vec![None; item.sockets.len()],
        stickers: Vec::new(),
    };
    (!art.parts().is_empty()).then_some(art)
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
            let tail = input.shortfall("not in pack");
            p.put_to(x0 + 45, y, &tail, fg(MAGENTA).bg(bg), l.divider);
        }
    }
}

/// The title block: the recipe and where it sits in the set, what it makes (for an
/// assembly, the item's kind, slot and weight), the skill it needs, how much of it is
/// on hand, and the drawing's number and kind.
fn title_block(
    p: &mut Pen,
    l: &Layout,
    st: &AppState,
    r: &Recipe,
    ins: &[Input],
    (i, n): (usize, usize),
    assembly: bool,
) {
    let x = l.title;
    let y = l.rule + 1;
    let edge = l.right;
    // Every line ends one column inside the sheet's edge, cut with `…` when longer.
    let room = (edge - x).max(0) as usize;
    let mut line = |dy: i32, text: &str, style: Style| {
        p.put_to(x, y + dy, &cut(text, room), style, edge);
    };
    let index = format!("◂ {}/{n} ▸", i + 1);
    let ix = l.right - 3 - width(&index);
    let name_room = (ix - 1 - x).max(0) as usize;
    let name = cut(&r.name.to_uppercase(), name_room);
    line(0, &name, bold(CYAN));

    // What it makes, without the kind when the whole line doesn't fit.
    let (out, count) = (&r.output.0, r.output.1);
    let makes = match st.content.item(out) {
        Some(item) if assembly => {
            let mut bits = vec![item.kind.label().to_string()];
            bits.extend(item.slot.map(|s| s.label().to_string()));
            bits.push(format!("{:.1} kg", item.weight));
            bits.join(" · ")
        }
        Some(item) => {
            let whole = format!("makes {count}× {} · {}", item.name, item.kind.label());
            if width(&whole) as usize <= room {
                whole
            } else {
                format!("makes {count}× {}", item.name)
            }
        }
        None => format!("makes {count}× {out}"),
    };
    line(1, &makes, fg(DIM));

    let skill = skill_name(st, r);
    let needs = if st.character.has_skill(&r.skill) {
        (format!("needs {skill} · yours"), DIM)
    } else {
        (format!("needs {skill}"), MAGENTA)
    };
    line(2, &needs.0, fg(needs.1));

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
    line(3, &hand.0, fg(hand.1));

    let dwg = format!(
        "DWG {}/{n} · {} · drawn {}",
        i + 1,
        if assembly { "assembly" } else { "process" },
        st.character.handle.to_uppercase()
    );
    line(4, &dwg, fg(FAINTER));
    p.put_to(ix, y, &index, fg(DIM), edge);
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
fn process(p: &mut Pen, st: &AppState, r: &Recipe, ins: &[Input], (sel, t): (usize, Option<u64>)) {
    let f = p.clip;
    if f.is_empty() {
        return;
    }
    let (fx, fy, fh) = (f.x as i32, f.y as i32, f.height as i32);
    // Laid out for `PROCESS_FIELD`, the diagram runs from the balloons, four left of
    // the boxes, to the result box's edge, a column in from the field's. A wider field
    // moves it right until it sits centred.
    let lead = BOX_X - 4;
    let span = PROCESS_FIELD - lead - 1;
    let fw = i32::from(f.width);
    let fx = fx + ((fw - span) / 2 - lead).max(0);
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
    // The three runs light as the build passes thirds of the closing, and stay lit.
    let lit = |k: u64| t.is_some_and(|t| t * 3 >= k * CLOSE_TICKS);
    let run = |k: u64| fg(if lit(k) { CYAN } else { FAINTER });
    let faint = run(1);
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
    // The steps sit two in from the edge, one when that leaves no space after the
    // longest; the box widens only when a space each side won't fit.
    let longest = steps.iter().map(|s| width(s)).max().unwrap_or(0);
    let w = PROCESS_W.max(longest + 4).max(width(&skill) + 4);
    let inset = if longest + 5 <= w { 3 } else { 2 };
    let (px0, px1) = (jx + 7, jx + 6 + w);
    p.outline(
        (px0, mid - 3, px1, mid + 3),
        DOUBLE,
        if lit(2) { CYAN } else { DIM },
    );
    p.put(
        px0 + (px1 - px0 - width(&skill)) / 2 + 1,
        mid - 3,
        &skill,
        bold(CYAN),
    );
    for (i, s) in steps.iter().enumerate() {
        p.put_to(px0 + inset, mid - 1 + i as i32, s, fg(TEXT), px1);
    }

    // Out to the result: the icon, then how many of what, wrapped onto as many lines
    // as it needs, the box growing round the middle row.
    p.put(px1 + 1, mid, "────▶", run(3));
    let ox0 = px1 + 7;
    let ox1 = fx + fw.min(PROCESS_FIELD) - 2;
    if ox1 <= ox0 + 2 {
        return;
    }
    let (out, count) = (&r.output.0, r.output.1);
    let item = st.content.item(out);
    let icon = item.map_or('?', |i| kind_icon(i.kind));
    let name = item.map_or_else(|| out.to_string(), |i| i.name.clone());
    let room = (ox1 - 1 - (ox0 + 4)).max(0) as usize;
    let lines = wrap(&format!("{count}× {name}"), room);
    let rows = lines.len().max(1) as i32;
    let first = mid - (rows - 1) / 2;
    p.outline((ox0, first - 1, ox1, first + rows), LIGHT, CYAN);
    p.ch(ox0 + 2, first, icon, bold(CYAN));
    for (i, line) in lines.iter().enumerate() {
        p.put_to(ox0 + 4, first + i as i32, line, bold(CYAN), ox1);
    }
    if t.is_some_and(|t| t >= CLOSE_TICKS) {
        banner(p, " BUILT ", (ox0 + ox1) / 2, first + rows);
    }
}

/// A banner centred on column `mid` of row `y`: bold, the panel's colour on cyan.
fn banner(p: &mut Pen, text: &str, mid: i32, y: i32) {
    p.put(mid - width(text) / 2, y, text, bold(PANEL_BG).bg(CYAN));
}

/// `text` broken at spaces into lines of at most `room` cells; a word longer than
/// the room is cut with `…`.
fn wrap(text: &str, room: usize) -> Vec<String> {
    if room == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace().map(|w| cut(w, room)) {
        if line.is_empty() {
            line = word;
        } else if width(&line) + 1 + width(&word) <= room as i32 {
            line.push(' ');
            line.push_str(&word);
        } else {
            lines.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
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

/// Boxes as left, top, right and bottom, the last two exclusive.
type Span = (i32, i32, i32, i32);

/// A part on the exploded drawing: its place, its rectangle moved apart (in drawing
/// coordinates), and which input it is, if the recipe takes it.
struct Shown {
    place: PartPlace,
    rect: Span,
    input: Option<usize>,
}

impl Shown {
    fn new(place: PartPlace, ins: &[Input]) -> Shown {
        let (dx, dy) = (i32::from(place.apart.0), i32::from(place.apart.1));
        let (x, y) = (place.x + dx, place.y + dy);
        Shown {
            rect: (x, y, x + i32::from(place.w), y + i32::from(place.h)),
            input: ins.iter().position(|i| i.id.0 == place.item),
            place,
        }
    }

    fn apart(&self) -> (i32, i32) {
        (i32::from(self.place.apart.0), i32::from(self.place.apart.1))
    }

    /// The balloon's left cell, moved with the part, and how wide it is; none for a
    /// part the recipe doesn't take.
    fn balloon(&self) -> Option<(i32, i32, i32)> {
        let n = self.input? + 1;
        let (dx, dy) = self.apart();
        let (x, y) = self.place.balloon;
        Some((x + dx, y + dy, width(&format!(" {n} "))))
    }

    fn short(&self, ins: &[Input]) -> bool {
        self.input.is_some_and(|i| !ins[i].on_hand())
    }
}

/// The cells of a leader running `dir` for `len` cells from a balloon `w` wide at
/// `(x, y)`: from the middle of its top or bottom, or from its side.
fn leader(x: i32, y: i32, w: i32, (dir, len): (Dir, u8)) -> impl Iterator<Item = (i32, i32)> {
    let mid = x + (w - 1) / 2;
    (1..=i32::from(len)).map(move |k| match dir {
        Dir::Up => (mid, y - k),
        Dir::Down => (mid, y + k),
        Dir::Left => (x - k, y),
        Dir::Right => (x + w - 1 + k, y),
    })
}

/// The box round all of `boxes`.
fn bounds(boxes: impl IntoIterator<Item = Span>) -> Option<Span> {
    boxes
        .into_iter()
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

/// The origin that centres `b` in `f`, and whether `b` fits there.
fn centre(b: Span, f: Rect) -> ((i32, i32), bool) {
    let (w, h) = (b.2 - b.0, b.3 - b.1);
    let (fw, fh) = (i32::from(f.width), i32::from(f.height));
    let origin = (
        i32::from(f.x) + (fw - w) / 2 - b.0,
        i32::from(f.y) + (fh - h) / 2 - b.1,
    );
    (origin, w <= fw && h <= fh)
}

/// Where the drawing's origin goes for the exploded drawing's bounding box, every
/// moved part with its balloon and leader, to sit centred in `f`; none when it doesn't
/// fit, or a part would start off the buffer.
fn exploded_origin(shown: &[Shown], f: Rect) -> Option<(i32, i32)> {
    let mut boxes = Vec::new();
    for s in shown {
        boxes.push(s.rect);
        if let Some((x, y, w)) = s.balloon() {
            boxes.push((x, y, x + w, y + 1));
            boxes.extend(leader(x, y, w, s.place.leader).map(|(x, y)| (x, y, x + 1, y + 1)));
        }
    }
    let ((ox, oy), fits) = centre(bounds(boxes)?, f);
    let on_buffer = |v: i32| u16::try_from(v).is_ok();
    let placed = shown
        .iter()
        .map(Shown::apart)
        .all(|(dx, dy)| on_buffer(ox + dx) && on_buffer(oy + dy));
    (fits && placed).then_some((ox, oy))
}

/// The output drawn exploded in the field: each part moved by its `apart`, faint
/// centre lines through the gaps they leave, a balloon with its number on each part
/// the recipe takes, a part short as a ghost in a dashed box, and the selected part in
/// brackets. When that doesn't fit, the item is drawn whole, centred and clipped.
fn exploded(
    p: &mut Pen,
    st: &AppState,
    art: &ItemArt,
    ins: &[Input],
    (sel, t): (usize, Option<u64>),
) {
    let f = p.clip;
    if f.is_empty() {
        return;
    }
    let shown: Vec<Shown> = art
        .parts()
        .into_iter()
        .map(|place| Shown::new(place, ins))
        .collect();
    let Some((ox, oy)) = exploded_origin(&shown, f) else {
        // Whole: the parts where they sit, what the drawing draws with nothing fitted.
        let home = shown.iter().map(|s| {
            let pl = &s.place;
            (pl.x, pl.y, pl.x + i32::from(pl.w), pl.y + i32::from(pl.h))
        });
        if let Some(b) = bounds(home) {
            let ((ox, oy), _) = centre(b, f);
            let cell = |v: i32| u16::try_from(v.max(0)).unwrap_or(u16::MAX);
            art.draw((cell(ox), cell(oy)), f, p.buf);
            if t.is_some_and(|t| t >= CLOSE_TICKS) {
                banner(
                    p,
                    " ASSEMBLED ",
                    ox + (b.0 + b.2) / 2,
                    (oy + b.3).min(last_row(f)),
                );
            }
        }
        return;
    };
    if let Some(t) = t {
        close_up(p, st, art, &shown, (ox, oy), t);
        return;
    }
    // The centre lines first: a part drawn over one keeps its own cells.
    for s in &shown {
        centre_lines(p, s, (ox, oy));
    }
    for s in &shown {
        let (dx, dy) = s.apart();
        let origin = ((ox + dx) as u16, (oy + dy) as u16);
        if s.short(ins) {
            ghost(p, art, s.place.index, origin);
        } else {
            art.draw_part(s.place.index, origin, f, p.buf);
        }
    }
    // A part short in a dashed box a column wider than it, what it lacks under it; set
    // into the box's bottom edge when the row under it is past the field.
    for s in &shown {
        let Some(input) = s.input.map(|i| &ins[i]).filter(|i| !i.on_hand()) else {
            continue;
        };
        let (x0, y0, x1, y1) = s.rect;
        let (left, top, right, bottom) = (ox + x0 - 1, oy + y0, ox + x1, oy + y1 - 1);
        p.outline((left, top, right, bottom), DASHED, MAGENTA);
        let label = input.shortfall("none on hand");
        if bottom + 2 <= last_row(f) {
            p.put(left, bottom + 2, &label, fg(MAGENTA));
        } else {
            in_edge(p, &label, (left, right), bottom, fg(MAGENTA));
        }
    }
    for s in &shown {
        let (Some(i), Some((x, y, w))) = (s.input, s.balloon()) else {
            continue;
        };
        let ok = ins[i].on_hand();
        let ink = fg(if ok { CYAN } else { MAGENTA });
        let glyph = match s.place.leader.0 {
            Dir::Up | Dir::Down => '│',
            Dir::Left | Dir::Right => '─',
        };
        for (lx, ly) in leader(x, y, w, s.place.leader) {
            p.ch(ox + lx, oy + ly, glyph, ink);
        }
        balloon(p, ox + x, oy + y, i + 1, ok, i == sel);
    }
    if let Some(s) = shown.iter().find(|s| s.input == Some(sel)) {
        let (x0, y0, x1, y1) = s.rect;
        let (cx, cy) = BRACKET_ROOM;
        let room = (ox + x0 - cx, oy + y0 - cy, ox + x1 + cx, oy + y1 + cy);
        brackets(p.buf, inside_corners(room, f), st.tick, f);
    }
}

/// `text` set into a box's edge on row `y` between corner columns `left` and `right`,
/// centred: a space each side when there is room, cut to the edge when it is short.
fn in_edge(p: &mut Pen, text: &str, (left, right): (i32, i32), y: i32, style: Style) {
    let room = (right - left - 1).max(0);
    let text = if width(text) + 2 <= room {
        format!(" {text} ")
    } else {
        cut(text, room as usize)
    };
    p.put(left + 1 + (room - width(&text)) / 2, y, &text, style);
}

/// The field's last row.
fn last_row(f: Rect) -> i32 {
    i32::from(f.bottom()) - 1
}

/// The parts closing on the drawing's origin `(ox, oy)` as the build runs `t` ticks,
/// all in colour with nothing else drawn; then the item whole, in brackets with its
/// banner under it.
fn close_up(
    p: &mut Pen,
    st: &AppState,
    art: &ItemArt,
    shown: &[Shown],
    (ox, oy): (i32, i32),
    t: u64,
) {
    let f = p.clip;
    let cell = |v: i32| u16::try_from(v.max(0)).unwrap_or(u16::MAX);
    if t < CLOSE_TICKS {
        let left = 1.0 - Easing::InOutCubic.apply(t as f32 / CLOSE_TICKS as f32);
        for s in shown {
            let (dx, dy) = s.apart();
            let step = |v: i32| (v as f32 * left).round() as i32;
            let origin = (cell(ox + step(dx)), cell(oy + step(dy)));
            art.draw_part(s.place.index, origin, f, p.buf);
        }
        return;
    }
    art.draw((cell(ox), cell(oy)), f, p.buf);
    let home = shown.iter().map(|s| {
        let pl = &s.place;
        (pl.x, pl.y, pl.x + i32::from(pl.w), pl.y + i32::from(pl.h))
    });
    let Some((x0, y0, x1, y1)) = bounds(home) else {
        return;
    };
    let (x0, y0, x1, y1) = (ox + x0, oy + y0, ox + x1, oy + y1);
    let (cx, cy) = BRACKET_ROOM;
    let room = (x0 - cx, y0 - cy, x1 + cx, y1 + cy);
    brackets(p.buf, inside_corners(room, f), st.tick, f);
    banner(p, " ASSEMBLED ", (x0 + x1) / 2, y1.min(last_row(f)));
}

/// The rectangle to hand `brackets` for corners round `room` that never fall past the
/// field: a corner that would lie outside `f` comes in to its edge row or column, so
/// all four always show.
fn inside_corners((left, top, right, bottom): Span, f: Rect) -> Rect {
    let (fx0, fy0) = (i32::from(f.x), i32::from(f.y));
    let (fx1, fy1) = (i32::from(f.right()) - 1, i32::from(f.bottom()) - 1);
    let inside = |v: i32, lo: i32, hi: i32| v.min(hi).max(lo);
    // The corners' own columns and rows.
    let (cl, cr) = (inside(left - 1, fx0, fx1), inside(right, fx0, fx1));
    let (ct, cb) = (inside(top - 1, fy0, fy1), inside(bottom, fy0, fy1));
    Rect::new(
        (cl + 1) as u16,
        (ct + 1) as u16,
        (cr - cl - 1).max(0) as u16,
        (cb - ct - 1).max(0) as u16,
    )
}

/// Faint centre lines through the gap a part left, in the cells its rectangle had
/// whole and has no longer: along its middle row when it moved sideways, down its
/// middle column when it moved up or down.
fn centre_lines(p: &mut Pen, s: &Shown, (ox, oy): (i32, i32)) {
    let pl = &s.place;
    let (x0, y0, x1, y1) = s.rect;
    let left = |x: i32, y: i32| !((x0..x1).contains(&x) && (y0..y1).contains(&y));
    let (dx, dy) = s.apart();
    let faint = fg(FAINTER);
    if dx != 0 {
        let y = pl.y + (i32::from(pl.h) - 1) / 2;
        for x in (pl.x..pl.x + i32::from(pl.w)).filter(|&x| left(x, y)) {
            p.ch(ox + x, oy + y, '╌', faint);
        }
    }
    if dy != 0 {
        let x = pl.x + (i32::from(pl.w) - 1) / 2;
        for y in (pl.y..pl.y + i32::from(pl.h)).filter(|&y| left(x, y)) {
            p.ch(ox + x, oy + y, '╎', faint);
        }
    }
}

/// A part short: drawn alone on a scratch buffer, then copied in greyed darker than
/// the modding mode's grey, without bold, its text blanked. Where it leaves its own
/// background the sheet's shows through.
fn ghost(p: &mut Pen, art: &ItemArt, part: usize, origin: (u16, u16)) {
    let f = p.clip;
    let mut scratch = Buffer::empty(f);
    art.draw_part(part, origin, f, &mut scratch);
    for y in f.top()..f.bottom() {
        for x in f.left()..f.right() {
            let c = &scratch[(x, y)];
            if *c == Cell::EMPTY {
                continue;
            }
            let symbol = if PIXELS.contains(&c.symbol()) {
                c.symbol()
            } else {
                " "
            };
            let fg = grey_at(c.fg, Color::Reset, GHOST);
            let bg = match c.bg {
                Color::Reset => p.buf[(x, y)].bg,
                bg => grey_at(bg, PANEL_BG, GHOST),
            };
            let cell = &mut p.buf[(x, y)];
            cell.reset();
            cell.set_symbol(symbol).set_fg(fg).set_bg(bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::deck::{Deck, Tab};
    use crate::screens::testutil::key;
    use crate::ui::art::{Art, grey};
    use crate::ui::chrome::{BAR_BG, CYAN, MAGENTA, PANEL_BG};
    use crate::ui::looks::ItemArt;
    use crossterm::event::KeyCode;
    use game::{Character, Content, ItemId, RecipeId, SkillId};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::{Buffer, Cell};
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

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

    /// A process sheet's result box width, then the room the diagram leaves in the
    /// field on its left and on its right.
    fn process_geometry(b: &Buffer) -> (u16, u16, u16) {
        let (w, h) = (b.area.width, b.area.height);
        let top = (0..h).find(|&y| b[(2, y)].symbol() == "┌").unwrap();
        let rule = (top..h).find(|&y| b[(w - 3, y)].symbol() == "┤").unwrap();
        // The field's columns, the sheet's side edges outside them.
        let (x0, x1) = (3, w - 3);
        let drawn: Vec<(u16, u16)> = (top + 1..rule)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&at| b[at].symbol() != " " || b[at].bg != PANEL_BG)
            .collect();
        let left = drawn.iter().map(|at| at.0).min().unwrap();
        let right = drawn.iter().map(|at| at.0).max().unwrap();
        // The result box is the rightmost: its top right corner, then its top left.
        let (cx, cy) = drawn
            .iter()
            .copied()
            .filter(|&at| b[at].symbol() == "┐")
            .max_by_key(|at| at.0)
            .unwrap();
        let ox0 = (x0..cx)
            .rev()
            .find(|&x| b[(x, cy)].symbol() == "┌")
            .unwrap();
        (cx - ox0 + 1, left - x0, x1 - 1 - right)
    }

    #[test]
    fn a_process_keeps_its_size_and_is_centred_on_a_wide_sheet() {
        let mut st = dshawn();
        let mut d = bench(&mut st);
        let mut seen = 0;
        for &i in &drawings(&st.content) {
            if assembly(&st, &st.content.recipes[i]).is_none() {
                seen += 1;
                let id = recipe_id(&st, &d);
                let (box_w, ..) = process_geometry(&render(&d, &st, 100, 30));
                for (w, h) in [(120, 40), (200, 50)] {
                    let (wide_w, left, right) = process_geometry(&render(&d, &st, w, h));
                    assert_eq!(wide_w, box_w, "{id} at {w}×{h}");
                    assert!(
                        left.abs_diff(right) <= 1,
                        "{id} at {w}×{h}: {left} left, {right} right"
                    );
                }
            }
            press(&mut d, &mut st, KeyCode::Right, 1);
        }
        assert_eq!(seen, 6);
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
        // Two servos held of four: two short, not missing from the pack.
        assert_eq!(cells(&b, 39, 21, 1), "2", "{}", row(&b, 21));
        assert_eq!(cells(&b, 48, 21, 11), "2 short    ", "{}", row(&b, 21));
        assert_eq!(b[(48, 21)].fg, MAGENTA);
        assert_eq!(b[(45, 21)].symbol(), "□");
        assert_eq!(cells(&b, 48, 22, 11), " ".repeat(11), "{}", row(&b, 22));
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
            "←→ drawings · ↑↓ parts · space build · q/e tabs · tab close"
        );
        // e still goes to the next tab and q back; tab closes.
        press(&mut d, &mut st, KeyCode::Char('e'), 1);
        assert_eq!(d.tab, Tab::Jobs);
        press(&mut d, &mut st, KeyCode::Char('q'), 1);
        assert_eq!(d.tab, Tab::Workbench);
        assert_eq!(recipe_id(&st, &d), "hardhat_visor", "the sheet stays put");
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Tab)]),
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
            anim: None,
        }
        .pack();
        let clamped = Deck::new(6).restore(&st);
        assert_eq!(
            clamped.workbench,
            Workbench {
                drawing: 6,
                part: 0,
                anim: None
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

        // The drawing number is cut with `…`, ending one column inside the edge.
        let shown: String = dwg.chars().take(33).chain(['…']).collect();
        let b = render(&d, &st, 100, 30);
        assert_eq!(b[(97, 25)].symbol(), "│", "{}", row(&b, 25));
        assert_eq!(cells(&b, 63, 25, 34), shown, "{}", row(&b, 25));
        assert_eq!(b[(96, 25)].symbol(), "…", "{}", row(&b, 25));

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
        assert_eq!(cells(&b, 83, rule + 5, 34), shown, "{}", row(&b, rule + 5));
        assert_eq!(b[(117, rule + 5)].symbol(), "│", "{}", row(&b, rule + 5));
    }

    #[test]
    fn a_long_result_wraps_in_its_box() {
        let mut st = dshawn();
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Right, 2);
        assert_eq!(recipe_id(&st, &d), "optics_tune");
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        // Two lines in one box, the icon on the first, grown a row under the middle.
        assert_eq!(b[(79, 11)].symbol(), "◈", "{field}");
        assert_eq!(cells(&b, 81, 11, 13), "1× Nightglass", "{field}");
        assert_eq!(cells(&b, 79, 12, 8), "  optics", "{field}");
        for (x, y, corner) in [(77, 10, "┌"), (95, 10, "┐"), (77, 13, "└"), (95, 13, "┘")] {
            assert_eq!(b[(x, y)].symbol(), corner, "({x}, {y}):\n{field}");
        }
        for y in [11, 12] {
            assert_eq!(b[(77, y)].symbol(), "│", "{field}");
            assert_eq!(b[(95, y)].symbol(), "│", "{field}");
        }
        assert_eq!(cells(&b, 71, 11, 5), "────▶", "{field}");
        // The steps fill the box, a space each side.
        assert_eq!(cells(&b, 51, 10, 20), "║ 1 re-seat lenses ║", "{field}");
        // The kind goes when the whole line doesn't fit.
        assert_eq!(
            cells(&b, 63, 22, 34).trim_end(),
            "makes 1× Nightglass optics",
            "{}",
            row(&b, 22)
        );
    }

    #[test]
    fn title_lines_cut_inside_the_edge() {
        let mut st = dshawn();
        let mut r = st.content.recipes[0].clone();
        r.id = RecipeId::new("long_lines");
        r.skill = SkillId::new("scrapcraft");
        r.inputs = vec![(ItemId::new("a_part_with_an_id_far_too_long"), 2)];
        st.content.recipes.push(r);
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Left, 1);
        assert_eq!(recipe_id(&st, &d), "long_lines");
        let b = render(&d, &st, 100, 30);
        assert_eq!(
            cells(&b, 63, 24, 34),
            "2 more a_part_with_an_id_far_too_…",
            "{}",
            row(&b, 24)
        );
        assert_eq!(b[(97, 24)].symbol(), "│");
    }

    #[test]
    fn wrap_breaks_at_spaces_and_cuts_long_words() {
        assert_eq!(wrap("2× Stim patch", 13), ["2× Stim patch"]);
        assert_eq!(
            wrap("1× Nightglass optics", 13),
            ["1× Nightglass", "optics"]
        );
        assert_eq!(
            wrap("1× Hardhat with visor", 11),
            ["1× Hardhat", "with visor"]
        );
        assert_eq!(wrap("1× Unbreakableword", 8), ["1×", "Unbreak…"]);
        assert!(wrap("anything", 0).is_empty());
    }

    /// D'SHAWN's deck on the Ratchet's sheet, the last drawing: ← once from the first.
    fn on_the_ratchet(st: &mut AppState) -> Deck {
        let mut d = bench(st);
        press(&mut d, st, KeyCode::Left, 1);
        assert_eq!(recipe_id(st, &d), "ratchet_build");
        d
    }

    /// Where `concept_a` puts the drawing's origin at 100×30.
    const MOCK_ORIGIN: (u16, u16) = (14, 4);

    /// The field at 100×30.
    const FIELD: Rect = Rect::new(3, 6, 94, 14);

    /// The output of the sheet shown, drawn with nothing fitted and no stickers.
    fn output_art(st: &AppState, d: &Deck) -> ItemArt {
        let list = drawings(&st.content);
        let r = &st.content.recipes[list[d.workbench.drawing]];
        let item = st.content.item(&r.output.0).unwrap();
        ItemArt {
            look: item.look.unwrap(),
            fitted: vec![None; item.sockets.len()],
            stickers: Vec::new(),
        }
    }

    /// A 100×30 buffer on the panel, as the sheet lays its field.
    fn panel() -> Buffer {
        let mut b = Buffer::empty(Rect::new(0, 0, 100, 30));
        for c in &mut b.content {
            c.set_bg(PANEL_BG);
        }
        b
    }

    /// The cells `draw` writes over an empty 100×30 buffer, row by row.
    fn drawn(draw: impl FnOnce(&mut Buffer)) -> Vec<(u16, u16)> {
        let mut b = Buffer::empty(Rect::new(0, 0, 100, 30));
        draw(&mut b);
        (0..30u16)
            .flat_map(|y| (0..100u16).map(move |x| (x, y)))
            .filter(|&at| b[at] != Cell::EMPTY)
            .collect()
    }

    /// `at` moved by a part's `apart`.
    fn moved((x, y): (u16, u16), (dx, dy): (i16, i16)) -> (u16, u16) {
        (
            (i32::from(x) + i32::from(dx)) as u16,
            (i32::from(y) + i32::from(dy)) as u16,
        )
    }

    #[test]
    fn the_ratchet_explodes() {
        let mut st = dshawn();
        let d = on_the_ratchet(&mut st);
        let b = render(&d, &st, 100, 30);
        let title = area(&b, 62, 97, 21, 26);
        assert_eq!(cells(&b, 63, 21, 11), "RATCHET SMG", "{title}");
        assert_eq!(cells(&b, 87, 21, 7), "◂ 7/7 ▸", "{title}");
        assert_eq!(
            cells(&b, 63, 22, 34).trim_end(),
            "weapon · primary · 3.2 kg",
            "{title}"
        );
        assert_eq!(
            cells(&b, 63, 23, 34).trim_end(),
            "needs Scrapcraft · yours",
            "{title}"
        );
        assert_eq!(
            cells(&b, 63, 24, 34).trim_end(),
            "1 more Barrel 9×19",
            "{title}"
        );
        assert_eq!(
            cells(&b, 63, 25, 34),
            "DWG 7/7 · assembly · drawn D'SHAWN",
            "{title}"
        );
        assert_eq!(
            cells(&b, 2, 28, 63).trim_end(),
            "←→ drawings · ↑↓ parts · space assemble · q/e tabs · tab close"
        );

        // A balloon on each part with its leader, numbered as the parts list.
        let field = area(&b, 3, 97, 6, 20);
        for (n, x, y) in [(1, 38, 6), (2, 17, 6), (3, 83, 6), (4, 42, 17), (5, 77, 16)] {
            assert_eq!(cells(&b, x, y, 3), format!(" {n} "), "{field}");
            assert!(b[(x + 1, y)].modifier.contains(Modifier::BOLD), "{n}");
        }
        for (x, y, glyph) in [
            (39, 7, "│"),
            (18, 7, "│"),
            (84, 7, "│"),
            (45, 17, "─"),
            (46, 17, "─"),
            (47, 17, "─"),
            (75, 16, "─"),
            (76, 16, "─"),
        ] {
            assert_eq!(b[(x, y)].symbol(), glyph, "({x}, {y}):\n{field}");
        }
        // The receiver is selected: its balloon inverted. The stock's is cyan on the
        // bar's colour, the barrel's (short) magenta on dark magenta, its leader too.
        assert_eq!((b[(39, 6)].fg, b[(39, 6)].bg), (PANEL_BG, CYAN));
        assert_eq!((b[(18, 6)].fg, b[(18, 6)].bg), (CYAN, BAR_BG));
        assert_eq!((b[(84, 6)].fg, b[(84, 6)].bg), (MAGENTA, WARN_BG));
        assert_eq!(b[(18, 7)].fg, CYAN);
        assert_eq!(b[(84, 7)].fg, MAGENTA);

        // Each part on hand sits where the whole drawing has it from the mock's
        // origin, moved by its `apart`.
        let art = output_art(&st, &d);
        let mut whole = panel();
        art.draw(MOCK_ORIGIN, FIELD, &mut whole);
        let parts = art.parts();
        assert_eq!(parts.len(), 5);
        for part in parts {
            if st.character.count(&ItemId::new(part.item)) == 0 {
                continue;
            }
            let mine = drawn(|buf| art.draw_part(part.index, MOCK_ORIGIN, FIELD, buf));
            let at = moved(mine[0], part.apart);
            assert_eq!(b[at], whole[mine[0]], "{} at {at:?}:\n{field}", part.item);
        }
        // The receiver doesn't move: its first cell, the charging knob's, is on (41, 7)
        // as in the mock.
        let first = drawn(|buf| art.draw_part(0, MOCK_ORIGIN, FIELD, buf))[0];
        assert_eq!(first, (41, 7));
        assert_eq!(b[(41, 7)].symbol(), "▀", "{field}");
    }

    #[test]
    fn a_missing_part_is_a_ghost_in_a_dashed_box() {
        let mut st = dshawn();
        let mut d = on_the_ratchet(&mut st);
        let art = output_art(&st, &d);
        let barrel = art.parts()[2];
        assert_eq!(st.character.count(&ItemId::new(barrel.item)), 0);
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);

        // Its cells greyed darker than the modding mode's grey, the text blanked; the
        // dashed box takes its top and bottom rows.
        let mut whole = panel();
        art.draw(MOCK_ORIGIN, FIELD, &mut whole);
        let mine = drawn(|buf| art.draw_part(2, MOCK_ORIGIN, FIELD, buf));
        let middle: Vec<_> = mine.iter().filter(|&&(_, y)| y == 10).collect();
        assert_eq!(middle.len(), 10, "{field}");
        for &&at in &middle {
            let (got, was) = (&b[moved(at, barrel.apart)], &whole[at]);
            let red = |c: Color| match c {
                Color::Rgb(r, _, _) => r,
                _ => panic!("{c:?}"),
            };
            assert!(red(got.fg) < red(grey(was.fg, PANEL_BG)), "{at:?}");
            assert!(red(got.fg) > 0, "{at:?}");
            assert!(" ▀▄█▌▐".contains(got.symbol()), "{at:?}");
            assert!(!got.modifier.contains(Modifier::BOLD), "{at:?}");
        }
        // As the mock has it: luminance × 0.26, the blue a touch up.
        assert_eq!(b[(79, 10)].symbol(), "▀", "{field}");
        assert_eq!(b[(79, 10)].fg, Color::Rgb(0x19, 0x19, 0x1b));
        assert_eq!(b[(79, 10)].bg, Color::Rgb(0x12, 0x12, 0x13));

        // The dashed box, a column out each side, and `none on hand` under it.
        for (x, y, glyph) in [(78, 9, "┌"), (89, 9, "┐"), (78, 11, "└"), (89, 11, "┘")] {
            assert_eq!(b[(x, y)].symbol(), glyph, "({x}, {y}):\n{field}");
            assert_eq!(b[(x, y)].fg, MAGENTA);
        }
        assert_eq!(cells(&b, 79, 9, 10), "╌".repeat(10), "{field}");
        assert_eq!(cells(&b, 79, 11, 10), "╌".repeat(10), "{field}");
        assert_eq!(b[(78, 10)].symbol(), "╎", "{field}");
        assert_eq!(b[(89, 10)].symbol(), "╎", "{field}");
        assert_eq!(cells(&b, 78, 13, 12), "none on hand", "{field}");
        assert_eq!(b[(78, 13)].fg, MAGENTA);

        // ↓↓ selects the barrel: brackets two columns and a row clear of its box's
        // rectangle, one more out, and its balloon inverted.
        press(&mut d, &mut st, KeyCode::Down, 2);
        assert_eq!(d.workbench.part, 2);
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        for (x, y, glyph) in [(76, 7, "┏"), (91, 7, "┓"), (76, 13, "┗"), (91, 13, "┛")] {
            assert_eq!(b[(x, y)].symbol(), glyph, "({x}, {y}):\n{field}");
            assert!(
                matches!(b[(x, y)].fg, Color::Rgb(r, _, bl) if bl > 150 && r < 40),
                "({x}, {y}) {:?}",
                b[(x, y)].fg
            );
        }
        assert_eq!(cells(&b, 83, 6, 3), " 3 ", "{field}");
        assert_eq!((b[(84, 6)].fg, b[(84, 6)].bg), (PANEL_BG, MAGENTA));
        assert_eq!((b[(39, 6)].fg, b[(39, 6)].bg), (CYAN, BAR_BG));
        assert_ne!(b[(35, 14)].symbol(), "┗", "the receiver's brackets go");
    }

    #[test]
    fn a_label_with_no_room_under_its_box_sits_in_the_bottom_edge() {
        let mut st = dshawn();
        let mag = ItemId::new("ratchet_mag");
        st.character.inventory.retain(|s| s.item != mag);
        let d = on_the_ratchet(&mut st);
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        // The magazine's box: its rect moved a row down, a column out each side, from
        // the origin (14, 4): columns 48-65, rows 14-19, the field's last row.
        assert_eq!(b[(48, 14)].symbol(), "┌", "{field}");
        assert_eq!(b[(48, 19)].symbol(), "└", "{field}");
        assert_eq!(b[(65, 19)].symbol(), "┘", "{field}");
        assert_eq!(
            cells(&b, 48, 19, 18),
            "└╌ none on hand ╌┘",
            "{}",
            row(&b, 19)
        );
        assert_eq!(b[(51, 19)].fg, MAGENTA);
        // The barrel's has room under it, as before.
        assert_eq!(cells(&b, 78, 13, 12), "none on hand", "{field}");
    }

    #[test]
    fn a_part_short_but_held_says_how_many_it_lacks() {
        let mut st = dshawn();
        // The build takes two barrels and the pack holds one.
        let barrel = ItemId::new("ratchet_barrel");
        let r = st
            .content
            .recipes
            .iter_mut()
            .find(|r| r.id.0 == "ratchet_build")
            .unwrap();
        r.inputs.iter_mut().find(|(id, _)| *id == barrel).unwrap().1 = 2;
        st.character.give(barrel, 1);
        let d = on_the_ratchet(&mut st);
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert_eq!(cells(&b, 78, 9, 1), "┌", "still a ghost:\n{field}");
        assert_eq!(cells(&b, 78, 13, 12), "1 short     ", "{field}");
        assert_eq!(b[(78, 13)].fg, MAGENTA);
        assert_eq!(cells(&b, 48, 23, 11), "1 short    ", "{}", row(&b, 23));
    }

    #[test]
    fn the_brackets_always_show_four_corners() {
        let mut st = dshawn();
        let mut d = on_the_ratchet(&mut st);
        // The balloons' cells, which no corner may take.
        let balloons = [(38, 6), (17, 6), (83, 6), (42, 17), (77, 16)];
        // Per part in input order: the corners clockwise from the top left. The field
        // holds rows 6-19, so a corner that would fall past it is brought in to its
        // edge row; the barrel's and the trigger's fit as they are.
        let corners: [[(u16, u16); 4]; 5] = [
            [(35, 6), (76, 6), (76, 14), (35, 14)],
            [(8, 6), (35, 6), (35, 16), (8, 16)],
            [(76, 7), (91, 7), (91, 13), (76, 13)],
            [(46, 12), (67, 12), (67, 19), (46, 19)],
            [(60, 12), (75, 12), (75, 18), (60, 18)],
        ];
        for (part, want) in corners.iter().enumerate() {
            assert_eq!(d.workbench.part, part);
            let b = render(&d, &st, 100, 30);
            let field = area(&b, 3, 97, 6, 20);
            for (&(x, y), glyph) in want.iter().zip(["┏", "┓", "┛", "┗"]) {
                assert_eq!(
                    b[(x, y)].symbol(),
                    glyph,
                    "part {part} ({x}, {y}):\n{field}"
                );
                for &(bx, by) in &balloons {
                    assert!(
                        y != by || !(bx..bx + 3).contains(&x),
                        "part {part}: ({x}, {y}) on a balloon"
                    );
                }
            }
            for (n, &(bx, by)) in balloons.iter().enumerate() {
                assert_eq!(cells(&b, bx, by, 3), format!(" {} ", n + 1), "{field}");
            }
            // The sheet's edge and the rule keep their own glyphs.
            assert!(!row(&b, 5).contains(['┏', '┓']), "{}", row(&b, 5));
            assert!(!row(&b, 20).contains(['┗', '┛']), "{}", row(&b, 20));
            press(&mut d, &mut st, KeyCode::Down, 1);
        }
    }

    #[test]
    fn centre_lines_fill_the_gaps() {
        let mut st = dshawn();
        st.character.give(ItemId::new("ratchet_barrel"), 1);
        let d = on_the_ratchet(&mut st);
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert_eq!(
            cells(&b, 63, 24, 34).trim_end(),
            "every part on hand",
            "{field}"
        );
        // The five columns the stock left and the five the barrel left, on the
        // barrel's middle row, and no more.
        let dashes: Vec<u16> = (3..97).filter(|&x| b[(x, 10)].symbol() == "╌").collect();
        assert_eq!(dashes, [33, 34, 35, 36, 37, 74, 75, 76, 77, 78], "{field}");
        assert_eq!(b[(33, 10)].fg, FAINTER);
        // The row the magazine and the trigger left, through their middle columns.
        let ticks: Vec<u16> = (3..97).filter(|&x| b[(x, 13)].symbol() == "╎").collect();
        assert_eq!(ticks, [56, 67], "{field}");
        assert_eq!(b[(56, 13)].fg, FAINTER);

        // With the barrel short, its box's edge takes the line's last column.
        let mut st = dshawn();
        let d = on_the_ratchet(&mut st);
        let b = render(&d, &st, 100, 30);
        assert_eq!(cells(&b, 74, 10, 5), "╌╌╌╌╎", "{}", row(&b, 10));
        assert_eq!(b[(78, 10)].fg, MAGENTA);
    }

    #[test]
    fn too_big_draws_whole() {
        let mut st = dshawn();
        let d = on_the_ratchet(&mut st);
        // A body a row short leaves a field of 13 rows; exploded the Ratchet needs 14.
        let body = Rect::new(1, 5, 98, 21);
        let mut b = Buffer::empty(Rect::new(0, 0, 100, 30));
        d.workbench.draw(&st, body, &mut b);
        let field = Rect::new(3, 6, 94, 13);
        let text = area(&b, 3, 97, 6, 19);
        assert_eq!(b[(3, 19)].symbol(), "─", "the rule is a row up");

        // The whole gun and nothing else: no balloon, no box. Its parts span columns
        // 2-69 and rows 3-14 of the drawing, centred in the field from (14, 3).
        let art = output_art(&st, &d);
        let mut whole = panel();
        art.draw((14, 3), field, &mut whole);
        let gun = drawn(|buf| art.draw((14, 3), Rect::new(0, 0, 100, 30), buf));
        assert!(
            gun.iter().all(|&at| field.contains(at.into())),
            "all in view"
        );
        for y in field.top()..field.bottom() {
            for x in field.left()..field.right() {
                let (got, want) = (&b[(x, y)], &whole[(x, y)]);
                assert_eq!(
                    (got.symbol(), got.fg, got.bg),
                    (want.symbol(), want.fg, want.bg),
                    "({x}, {y}):\n{text}"
                );
            }
        }
        assert!(text.contains("RATCHET"), "{text}");
        assert!(!text.contains(" 1 "), "{text}");
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

    /// The newest log line's text and whether it is a warning.
    fn latest(st: &AppState) -> (String, bool) {
        let e = st.log.latest().expect("a log line");
        (e.text.clone(), e.kind == crate::messages::Kind::Warn)
    }

    /// The Ratchet's sheet with every part in the pack, `tick` set to a round number.
    fn ready_ratchet() -> (AppState, Deck) {
        let mut st = dshawn();
        st.character.give(ItemId::new("ratchet_barrel"), 1);
        st.tick = 100;
        let d = on_the_ratchet(&mut st);
        (st, d)
    }

    #[test]
    fn space_assembles_and_the_parts_close() {
        let (mut st, mut d) = ready_ratchet();
        let smg = ItemId::new("ratchet_smg");
        let had = st.character.count(&smg);
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        // The log keeps the backticks as marks, not text.
        assert_eq!(latest(&st), ("built Ratchet SMG".to_string(), false));
        assert_eq!(st.character.count(&smg), had + 1);
        assert_eq!(st.character.count(&ItemId::new("ratchet_barrel")), 0);
        let copy = st
            .character
            .inventory
            .iter()
            .rev()
            .find(|s| s.item == smg)
            .and_then(|s| s.copy.as_ref())
            .expect("a copy");
        assert!(copy.fitted.iter().all(Option::is_none));

        // Seven ticks in the parts are part way home and no balloon shows.
        st.tick = 107;
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        for (n, x, y) in [(1, 38, 6), (2, 17, 6), (3, 83, 6), (4, 42, 17), (5, 77, 16)] {
            assert_ne!(cells(&b, x, y, 3), format!(" {n} "), "{field}");
        }
        let art = output_art(&st, &d);
        let mut whole = panel();
        art.draw(MOCK_ORIGIN, FIELD, &mut whole);
        let stock = art.parts()[1];
        let mine = drawn(|buf| art.draw_part(stock.index, MOCK_ORIGIN, FIELD, buf));
        let e = crate::anim::Easing::InOutCubic.apply(7.0 / CLOSE_TICKS as f32);
        let step = |v: i16| (f32::from(v) * (1.0 - e)).round() as i16;
        let (dx, dy) = (step(stock.apart.0), step(stock.apart.1));
        assert!(
            dx.abs() < stock.apart.0.abs() && dx.abs() > 0,
            "{dx} of {}",
            stock.apart.0
        );
        assert_eq!(
            b[moved(mine[0], (dx, dy))],
            whole[mine[0]],
            "stock part way:\n{field}"
        );
        assert!(!field.contains("ASSEMBLED"), "{field}");

        // Twenty ticks in it is whole, with its banner.
        st.tick = 120;
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert!(field.contains(" ASSEMBLED "), "{field}");
        let at = (3..97)
            .flat_map(|x| (6..20).map(move |y| (x, y)))
            .find(|&(x, y)| cells(&b, x, y, 11) == " ASSEMBLED ")
            .unwrap();
        assert_eq!((b[at].fg, b[at].bg), (PANEL_BG, CYAN));
        assert!(b[at].modifier.contains(Modifier::BOLD));
        assert_eq!(b[mine[0]], whole[mine[0]], "home:\n{field}");

        // Forty-six ticks in it is over: the sheet shows what is short.
        st.tick = 146;
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert!(!field.contains("ASSEMBLED"), "{field}");
        let parts = area(&b, 3, 61, 21, 26);
        assert!(parts.contains("not in pack"), "{parts}");
    }

    #[test]
    fn the_sheet_shows_the_counts_from_before_until_it_is_over() {
        let (mut st, mut d) = ready_ratchet();
        let r = st.content.recipes[drawings(&st.content)[d.workbench.drawing]].clone();
        let counts = |st: &AppState| -> Vec<String> {
            r.inputs
                .iter()
                .map(|(id, _)| st.character.count(id).to_string())
                .collect()
        };
        // The ON HAND column of the five rows.
        let shown = |b: &Buffer| -> Vec<String> {
            (21..26)
                .map(|y| cells(b, 39, y, 5).trim_end().to_string())
                .collect()
        };
        let before = counts(&st);
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        let after = counts(&st);
        assert_ne!(before, after);

        // Twenty ticks in, the item whole: the sheet still reads as it did.
        st.tick = 120;
        let b = render(&d, &st, 100, 30);
        let parts = area(&b, 3, 61, 21, 26);
        assert!(area(&b, 3, 97, 6, 20).contains(" ASSEMBLED "));
        assert_eq!(shown(&b), before, "{parts}");
        assert!(!parts.contains("not in pack"), "{parts}");
        assert_eq!(cells(&b, 63, 24, 18), "every part on hand");

        // Forty-six ticks in, it is over and the sheet shows the pack as it is.
        st.tick = 146;
        let b = render(&d, &st, 100, 30);
        let parts = area(&b, 3, 61, 21, 26);
        assert_eq!(shown(&b), after, "{parts}");
        assert!(parts.contains("not in pack"), "{parts}");
    }

    #[test]
    fn a_process_shows_the_counts_from_before_until_it_is_over() {
        let mut st = dshawn();
        st.character.skills.insert(SkillId::new("medtech"));
        // One coil, which the build takes.
        let coils = ItemId::new("copper_coil");
        st.character.inventory.retain(|s| s.item != coils);
        st.character.give(coils.clone(), 1);
        st.tick = 100;
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Right, 5);
        assert_eq!(recipe_id(&st, &d), "stim_batch");
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        assert_eq!(st.character.count(&coils), 0);

        // While BUILT shows, the coil's box is solid and the pack held one.
        st.tick = 120;
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert!(field.contains(" BUILT "), "{field}");
        assert_eq!(cells(&b, 13, 7, 3), "───", "{field}");
        assert_eq!(cells(&b, 39, 21, 1), "1", "{}", row(&b, 21));
        assert!(!row(&b, 21).contains("not in pack"), "{}", row(&b, 21));

        // Over, it is dashed and none is left.
        st.tick = 146;
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert_eq!(cells(&b, 13, 7, 3), "╌╌╌", "{field}");
        assert_eq!(cells(&b, 39, 21, 1), "0", "{}", row(&b, 21));
        assert!(row(&b, 21).contains("not in pack"), "{}", row(&b, 21));
    }

    #[test]
    fn space_again_ends_it_and_builds_nothing() {
        let (mut st, mut d) = ready_ratchet();
        let smg = ItemId::new("ratchet_smg");
        let had = st.character.count(&smg);
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        st.tick = 105;
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        assert_eq!(st.character.count(&smg), had + 1);
        assert_eq!(d.workbench.anim, None);
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert!(!field.contains("ASSEMBLED"), "{field}");
        assert_eq!(cells(&b, 17, 6, 3), " 2 ", "the sheet is back:\n{field}");

        // Esc ends it too, and leaves the deck open.
        for part in ["barrel", "receiver", "stock", "trigger"] {
            st.character
                .give(ItemId::new(&format!("ratchet_{part}")), 1);
        }
        st.tick = 200;
        press(&mut d, &mut st, KeyCode::Enter, 1);
        assert!(d.workbench.anim.is_some());
        st.tick = 205;
        let cmd = d.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(!matches!(cmd, ScreenCmd::Pop { .. }));
        assert_eq!(d.workbench.anim, None);
        assert_eq!(d.tab, Tab::Workbench);
        // With nothing running, esc closes as before.
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn a_refusal_starts_nothing() {
        let mut st = dshawn();
        st.tick = 100;
        let mut d = on_the_ratchet(&mut st);
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        assert_eq!(latest(&st), ("needs 1 more Barrel 9×19".to_string(), true));
        assert_eq!(d.workbench.anim, None);
        st.tick = 107;
        let b = render(&d, &st, 100, 30);
        assert!(!area(&b, 3, 97, 6, 20).contains("ASSEMBLED"));
    }

    #[test]
    fn a_process_builds_and_lights() {
        let mut st = dshawn();
        st.character.skills.insert(SkillId::new("medtech"));
        st.character.give(ItemId::new("copper_coil"), 1);
        st.tick = 100;
        let mut d = bench(&mut st);
        press(&mut d, &mut st, KeyCode::Right, 5);
        assert_eq!(recipe_id(&st, &d), "stim_batch");
        let stims = ItemId::new("stim_patch");
        let had = st.character.count(&stims);
        let before = render(&d, &st, 100, 30);
        assert_eq!(before[(51, 8)].fg, DIM, "the process box rests");
        press(&mut d, &mut st, KeyCode::Char(' '), 1);
        assert_eq!(st.character.count(&stims), had + 2);

        // A third in, the lines in are lit and the rest not yet.
        st.tick = 106;
        let b = render(&d, &st, 100, 30);
        assert_eq!(b[(40, 8)].fg, CYAN, "{}", row(&b, 8));
        assert_eq!(b[(51, 8)].fg, DIM);
        assert!(!area(&b, 3, 97, 6, 20).contains("BUILT"));

        st.tick = 120;
        let b = render(&d, &st, 100, 30);
        let field = area(&b, 3, 97, 6, 20);
        assert!(field.contains(" BUILT "), "{field}");
        // Every run is lit; the result box's edges are cyan with the banner on its
        // bottom edge.
        assert_eq!(b[(51, 8)].fg, CYAN);
        assert_eq!(b[(40, 8)].fg, CYAN);
        assert_eq!(b[(72, 11)].fg, CYAN, "{}", row(&b, 11));
        for (x, y) in [(77, 10), (95, 10), (77, 12), (95, 12)] {
            assert_eq!(b[(x, y)].fg, CYAN, "({x}, {y})");
        }
        let at = (77..96u16)
            .find(|&x| cells(&b, x, 12, 7) == " BUILT ")
            .unwrap();
        assert_eq!((b[(at, 12)].fg, b[(at, 12)].bg), (PANEL_BG, CYAN));

        st.tick = 146;
        let b = render(&d, &st, 100, 30);
        assert!(!area(&b, 3, 97, 6, 20).contains("BUILT"));
        assert_eq!(b[(51, 8)].fg, DIM);
    }
}
