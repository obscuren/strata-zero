//! The run's tether: a program in flight in a calm construct hangs off its target on
//! the map, a wire to a small label. The wire is the progress (a stretch lights for
//! each turn landed); a pulse rides it from the label each beat and the roll comes as
//! it reaches the target; a landing turns it green and reels it into the label.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::{CYAN, DIM, MAGENTA};
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::cmp::Reverse;

/// Ticks a landed run's tether stays: the first half whole, the second reeling in.
pub const LAND_TICKS: u64 = crate::net::BEAT_TICKS;

const UNLIT: Color = Color::Rgb(35, 100, 110);
const FRESH: Color = Color::Rgb(255, 255, 255);
const PULSE: Color = Color::Rgb(230, 253, 255);
/// The cell behind the pulse.
const WAKE: Color = Color::Rgb(127, 243, 255);
const GOOD: Color = Color::Rgb(80, 230, 120);
const GOOD_BG: Color = Color::Rgb(15, 58, 31);
const LABEL_BG: Color = Color::Rgb(10, 11, 16);
/// What a side costs when it would cover you or a piece of ICE.
const BLOCKED: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TetherState {
    Running,
    /// A turn just landed: its stretch and the target flash.
    Hit,
    /// The roll slipped: the unlit wire goes magenta.
    Slip,
    /// The run landed `age` ticks ago.
    Landed {
        age: u64,
    },
}

/// What the tether reads this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct TetherView {
    pub target: Pos,
    pub me: Pos,
    /// Pieces of ICE standing in sight: never drawn over.
    pub keep: Vec<Pos>,
    /// A program on yourself: the wire leaves from you and `@` stays as it is.
    pub on_self: bool,
    /// `CRACK`.
    pub op: String,
    /// `CRACKED`, once it lands.
    pub done: String,
    pub odds: u8,
    /// `door lock`.
    pub name: String,
    /// Turns landed, of `work`.
    pub turns: u8,
    pub work: u8,
    /// How far the clock is toward the next beat, 0 to 1.
    pub phase: f32,
    pub state: TetherState,
}

impl TetherView {
    fn landed(&self) -> Option<u64> {
        match self.state {
            TetherState::Landed { age } => Some(age),
            _ => None,
        }
    }

    /// The label's width: the widest its first line gets (two spaces and up to
    /// `100%` after the op, or the past tense) or the name, so a landing keeps the side.
    fn width(&self) -> u16 {
        (self.op.chars().count() + 6)
            .max(self.done.chars().count())
            .max(self.name.chars().count()) as u16
    }
}

/// The wire's full length: four cells a turn, from eight to sixteen.
pub fn wire_cells(work: u8) -> u16 {
    (4 * work as u16).clamp(8, 16)
}

/// Cells of a `cells` wire lit by `turns` of `work`.
fn lit_cells(turns: u8, work: u8, cells: u16) -> u16 {
    let work = work.max(1) as u32;
    ((turns as u32 * cells as u32 * 2 + work) / (2 * work)) as u16
}

/// Where the tether goes on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The wire leaves the target to the left.
    pub left: bool,
    /// The wire's length in cells.
    pub cells: u16,
    /// The screen row of the label's first line.
    pub label_row: u16,
}

impl Layout {
    /// Screen x of wire cell `i`, counted from the cap's end, for a target at `tx`.
    fn wire_x(&self, tx: u16, i: u16) -> u16 {
        if self.left {
            tx - self.cells + i
        } else {
            tx + self.cells - i
        }
    }

    fn cap_x(&self, tx: u16) -> u16 {
        if self.left {
            tx - self.cells - 1
        } else {
            tx + self.cells + 1
        }
    }

    /// The label's left edge for a label `w` wide, one blank cell past the cap.
    fn label_x(&self, tx: u16, w: u16) -> u16 {
        if self.left {
            tx - self.cells - 2 - w
        } else {
            tx + self.cells + 3
        }
    }

    /// The gap between the cap and the label.
    fn gap_x(&self, tx: u16) -> u16 {
        if self.left {
            tx - self.cells - 2
        } else {
            tx + self.cells + 2
        }
    }
}

fn to_tile(area: Rect, origin: Pos, x: u16, y: u16) -> Pos {
    Pos::new(
        x as i32 - area.x as i32 + origin.x,
        y as i32 - area.y as i32 + origin.y,
    )
}

/// Every cell a layout covers: the wire, the cap, and the label with its gap on both rows.
fn covered(v: &TetherView, lay: &Layout, tx: u16, ty: u16) -> Vec<(u16, u16)> {
    let w = v.width();
    let mut out: Vec<(u16, u16)> = (0..lay.cells).map(|i| (lay.wire_x(tx, i), ty)).collect();
    out.push((lay.cap_x(tx), ty));
    let x0 = lay.label_x(tx, w).min(lay.gap_x(tx));
    for row in [ty, lay.label_row] {
        out.extend((x0..x0 + w + 1).map(|x| (x, row)));
    }
    out
}

/// The side the tether takes, and how long its wire is: the side that covers less of
/// the construct and never you or a piece in sight; a full wire before a shortened
/// one; on a tie, the side with more room. The label keeps a cell clear of the map's
/// edge. `None` when neither side has room for a cell a turn.
pub fn layout(
    v: &TetherView,
    area: Rect,
    origin: Pos,
    on_map: impl Fn(Pos) -> bool,
) -> Option<Layout> {
    let (tx, ty) = tile_to_screen(area, origin, v.target)?;
    let label_row = if ty > area.y { ty - 1 } else { ty + 1 };
    if label_row >= area.bottom() {
        return None;
    }
    let w = v.width();
    let full = wire_cells(v.work);
    let shortest = (v.work as u16).max(1);
    let rooms = [(true, tx - area.x), (false, area.right() - 1 - tx)];
    rooms
        .into_iter()
        .filter_map(|(left, room)| {
            let cells = room.checked_sub(3 + w)?.min(full);
            (cells >= shortest).then_some((
                Layout {
                    left,
                    cells,
                    label_row,
                },
                room,
            ))
        })
        .min_by_key(|(lay, room)| {
            let cost: u32 = covered(v, lay, tx, ty)
                .into_iter()
                .map(|(x, y)| {
                    let p = to_tile(area, origin, x, y);
                    let blocked = (p == v.me && !v.on_self) || v.keep.contains(&p);
                    if blocked { BLOCKED } else { on_map(p) as u32 }
                })
                .sum();
            (cost >= BLOCKED, Reverse(lay.cells), cost, Reverse(*room))
        })
        .map(|(lay, _)| lay)
}

fn set(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    ch: Option<char>,
    fg: Color,
    bg: Option<Color>,
    bold: bool,
) {
    let cell = &mut buf[(x, y)];
    if let Some(ch) = ch {
        cell.set_char(ch);
    }
    cell.set_fg(fg);
    if let Some(bg) = bg {
        cell.set_bg(bg);
    }
    cell.modifier.remove(Modifier::DIM);
    if bold {
        cell.modifier.insert(Modifier::BOLD);
    } else {
        cell.modifier.remove(Modifier::BOLD);
    }
}

/// Draws the tether where `lay` puts it. You and the pieces in sight are left alone.
pub fn paint(v: &TetherView, lay: &Layout, area: Rect, origin: Pos, buf: &mut Buffer) {
    let Some((tx, ty)) = tile_to_screen(area, origin, v.target) else {
        return;
    };
    let keep = |x: u16, y: u16| {
        let p = to_tile(area, origin, x, y);
        p == v.me || v.keep.contains(&p)
    };
    let landed = v.landed();
    let cells = lay.cells;
    let lit = lit_cells(v.turns, v.work, cells);
    let fresh = match v.state {
        TetherState::Hit => lit_cells(v.turns.saturating_sub(1), v.work, cells),
        _ => lit,
    };
    // Reeling in over the landing's second half, from the target back to the cap.
    let half = LAND_TICKS / 2;
    let shown = match landed {
        Some(age) if age >= half => {
            let left = LAND_TICKS.saturating_sub(age);
            (cells as u64 * left / half).min(cells as u64) as u16
        }
        _ => cells,
    };
    // The label and its gap, on the map's background.
    let w = v.width();
    let x0 = lay.label_x(tx, w);
    let block = lay.label_x(tx, w).min(lay.gap_x(tx));
    for row in [ty, lay.label_row] {
        for x in block..block + w + 1 {
            if !keep(x, row) {
                set(buf, x, row, Some(' '), DIM, Some(LABEL_BG), false);
            }
        }
    }
    let mut text = |s: &str, row: u16, parts: &[(usize, Color, bool)]| {
        let mut at = 0;
        for &(len, fg, bold) in parts {
            for ch in s.chars().skip(at).take(len) {
                let x = x0 + at as u16;
                if !keep(x, row) {
                    set(buf, x, row, Some(ch), fg, Some(LABEL_BG), bold);
                }
                at += 1;
            }
        }
    };
    match landed {
        Some(_) => text(
            &v.done,
            lay.label_row,
            &[(v.done.chars().count(), GOOD, true)],
        ),
        None => {
            let op = v.op.chars().count();
            let line = format!("{}  {}%", v.op, v.odds);
            let rest = line.chars().count() - op;
            text(
                &line,
                lay.label_row,
                &[(op, CYAN, true), (rest, DIM, false)],
            );
        }
    }
    text(&v.name, ty, &[(v.name.chars().count(), DIM, false)]);
    // The cap and the wire.
    let cap = if lay.left { '╺' } else { '╸' };
    let cap_x = lay.cap_x(tx);
    if !keep(cap_x, ty) {
        set(
            buf,
            cap_x,
            ty,
            Some(cap),
            if landed.is_some() { GOOD } else { CYAN },
            None,
            false,
        );
    }
    for i in 0..shown {
        let x = lay.wire_x(tx, i);
        if keep(x, ty) {
            continue;
        }
        let (ch, fg) = if landed.is_some() {
            ('━', GOOD)
        } else if i < lit {
            ('━', if i >= fresh { FRESH } else { CYAN })
        } else if v.state == TetherState::Slip {
            ('╌', MAGENTA)
        } else {
            ('╌', UNLIT)
        };
        set(buf, x, ty, Some(ch), fg, None, false);
    }
    // The pulse rides toward the target between rolls.
    if v.state == TetherState::Running && cells > 0 {
        let at = (v.phase.clamp(0.0, 1.0) * (cells - 1) as f32).round() as u16;
        let x = lay.wire_x(tx, at);
        if !keep(x, ty) {
            set(buf, x, ty, Some('█'), PULSE, None, false);
        }
        if at > 0 {
            let behind = lay.wire_x(tx, at - 1);
            if !keep(behind, ty) {
                buf[(behind, ty)].set_fg(WAKE);
            }
        }
    }
    // The target keeps its glyph; on yourself `@` stays as it is.
    if !v.on_self {
        let (fg, bg) = match v.state {
            TetherState::Hit => (LABEL_BG, Some(CYAN)),
            TetherState::Landed { .. } => (GOOD, Some(GOOD_BG)),
            TetherState::Running | TetherState::Slip => (CYAN, None),
        };
        set(buf, tx, ty, None, fg, bg, true);
    }
}

/// The tether where the map screen placed it; the screen also keeps the footer's RUN
/// row when no tether fits.
pub struct TetherOverlay<'a> {
    pub view: &'a TetherView,
    pub layout: Layout,
}

impl Overlay for TetherOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        paint(self.view, &self.layout, area, origin, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> TetherView {
        TetherView {
            target: Pos::new(33, 10),
            me: Pos::new(33, 9),
            keep: Vec::new(),
            on_self: false,
            op: "CRACK".into(),
            done: "CRACKED".into(),
            odds: 60,
            name: "door lock".into(),
            turns: 1,
            work: 3,
            phase: 0.0,
            state: TetherState::Running,
        }
    }

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 26,
    };

    fn construct_east(p: Pos) -> bool {
        p.x >= 30
    }

    /// The view painted where it goes, over a map of blanks with the target and you.
    fn painted(v: &TetherView) -> Buffer {
        let mut buf = Buffer::empty(AREA);
        buf[(v.target.x as u16, v.target.y as u16)].set_char('▪');
        buf[(v.me.x as u16, v.me.y as u16)].set_char('@');
        let lay = layout(v, AREA, Pos::new(0, 0), construct_east).expect("room");
        paint(v, &lay, AREA, Pos::new(0, 0), &mut buf);
        buf
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn cells_of(buf: &Buffer, y: u16, ch: &str) -> Vec<u16> {
        (0..buf.area.width)
            .filter(|x| buf[(*x, y)].symbol() == ch)
            .collect()
    }

    #[test]
    fn the_wire_is_four_cells_a_turn_between_eight_and_sixteen() {
        let lengths: Vec<u16> = (1..=5).map(wire_cells).collect();
        assert_eq!(lengths, [8, 8, 12, 16, 16]);
    }

    #[test]
    fn it_reaches_off_the_construct() {
        let lay = layout(&view(), AREA, Pos::new(0, 0), construct_east).unwrap();
        assert_eq!(
            lay,
            Layout {
                left: true,
                cells: 12,
                label_row: 9
            }
        );
    }

    #[test]
    fn it_takes_the_side_away_from_you() {
        let v = TetherView {
            target: Pos::new(40, 10),
            me: Pos::new(39, 10),
            ..view()
        };
        let lay = layout(&v, AREA, Pos::new(0, 0), |_| false).unwrap();
        assert!(!lay.left, "{lay:?}");
    }

    #[test]
    fn it_leaves_ice_alone_and_keeps_its_side_on_landing() {
        let v = TetherView {
            target: Pos::new(40, 10),
            me: Pos::new(40, 9),
            keep: vec![Pos::new(37, 10)],
            ..view()
        };
        let lay = layout(&v, AREA, Pos::new(0, 0), |_| false).unwrap();
        assert!(!lay.left, "{lay:?}");
        let landed = TetherView {
            state: TetherState::Landed { age: 0 },
            ..v
        };
        assert_eq!(layout(&landed, AREA, Pos::new(0, 0), |_| false), Some(lay));
    }

    #[test]
    fn it_shortens_to_fit_and_gives_up_when_it_cannot() {
        let v = TetherView {
            target: Pos::new(17, 10),
            me: Pos::new(17, 9),
            ..view()
        };
        let narrow = Rect::new(0, 0, 33, 26);
        let lay = layout(&v, narrow, Pos::new(0, 0), |_| false).unwrap();
        assert_eq!((lay.left, lay.cells), (true, 3));
        let v = TetherView {
            target: Pos::new(10, 10),
            me: Pos::new(10, 9),
            ..view()
        };
        assert_eq!(
            layout(&v, Rect::new(0, 0, 20, 26), Pos::new(0, 0), |_| false),
            None
        );
    }

    #[test]
    fn on_the_top_row_the_label_goes_under() {
        let v = TetherView {
            target: Pos::new(33, 0),
            me: Pos::new(33, 1),
            ..view()
        };
        let lay = layout(&v, AREA, Pos::new(0, 0), construct_east).unwrap();
        assert_eq!(lay.label_row, 1);
    }

    #[test]
    fn a_run_mid_way_paints_its_wire_label_and_pulse() {
        let buf = painted(&TetherView {
            phase: 1.0,
            ..view()
        });
        assert!(row(&buf, 9).contains("CRACK  60%"), "{:?}", row(&buf, 9));
        assert!(
            row(&buf, 10).contains("door lock   ╺"),
            "{:?}",
            row(&buf, 10)
        );
        let lit = cells_of(&buf, 10, "━");
        assert_eq!(lit, [21, 22, 23, 24]);
        assert!(lit.iter().all(|x| buf[(*x, 10)].fg == CYAN));
        assert_eq!(cells_of(&buf, 10, "╌").len(), 7);
        assert_eq!(cells_of(&buf, 10, "█"), [32]);
        let target = &buf[(33, 10)];
        assert_eq!(target.symbol(), "▪");
        assert_eq!(target.fg, CYAN);
        assert!(target.modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(33, 9)].symbol(), "@");
    }

    #[test]
    fn a_hit_flashes_its_stretch_and_the_target() {
        let buf = painted(&TetherView {
            turns: 2,
            state: TetherState::Hit,
            ..view()
        });
        let white: Vec<u16> = cells_of(&buf, 10, "━")
            .into_iter()
            .filter(|x| buf[(*x, 10)].fg == FRESH)
            .collect();
        assert_eq!(white, [25, 26, 27, 28]);
        assert_eq!(buf[(33, 10)].bg, CYAN);
        assert!(cells_of(&buf, 10, "█").is_empty());
    }

    #[test]
    fn a_slip_turns_the_unlit_wire_magenta() {
        let buf = painted(&TetherView {
            state: TetherState::Slip,
            ..view()
        });
        let unlit = cells_of(&buf, 10, "╌");
        assert_eq!(unlit.len(), 8);
        assert!(unlit.iter().all(|x| buf[(*x, 10)].fg == MAGENTA));
        assert!(cells_of(&buf, 10, "█").is_empty());
    }

    #[test]
    fn a_landing_goes_green_then_reels_in() {
        let at = |age| {
            painted(&TetherView {
                turns: 3,
                state: TetherState::Landed { age },
                ..view()
            })
        };
        let buf = at(0);
        assert!(row(&buf, 9).contains("CRACKED"), "{:?}", row(&buf, 9));
        let wire = cells_of(&buf, 10, "━");
        assert_eq!(wire.len(), 12);
        assert!(wire.iter().all(|x| buf[(*x, 10)].fg == GOOD));
        assert_eq!(buf[(33, 10)].fg, GOOD);
        assert_eq!(cells_of(&at(LAND_TICKS * 3 / 4), 10, "━").len(), 6);
        assert!(cells_of(&at(LAND_TICKS - 1), 10, "━").len() <= 1);
    }

    #[test]
    fn on_yourself_the_at_sign_is_left_alone() {
        let v = TetherView {
            target: Pos::new(40, 10),
            me: Pos::new(40, 10),
            on_self: true,
            op: "SHIELD".into(),
            done: "RAISED".into(),
            name: "deck".into(),
            work: 1,
            turns: 0,
            ..view()
        };
        let mut buf = Buffer::empty(AREA);
        buf[(40, 10)].set_char('@');
        let before = buf[(40, 10)].clone();
        let lay = layout(&v, AREA, Pos::new(0, 0), |_| false).unwrap();
        paint(&v, &lay, AREA, Pos::new(0, 0), &mut buf);
        assert_eq!(buf[(40, 10)], before);
        assert!(row(&buf, 9).contains("SHIELD  60%"), "{:?}", row(&buf, 9));
        assert_eq!(
            cells_of(&buf, 10, "╌").len() + cells_of(&buf, 10, "█").len(),
            8
        );
    }
}
