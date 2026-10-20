//! The turn column: who acts, in order, pinned to the map's top-right corner while
//! the map is on turns, with the live half of your turn lit.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::Overlay;
use crate::ui::chrome::CYAN;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// The column's width in cells.
pub const WIDTH: u16 = 18;
const BLACK: Color = Color::Rgb(10, 11, 16);
const DIM: Color = Color::Rgb(110, 110, 110);
const TEXT: Color = Color::Rgb(200, 200, 200);
const YOURS: Color = Color::Rgb(0, 20, 26);
/// The live half's band.
const LIT: Color = Color::Rgb(11, 70, 82);
const THEIRS: Color = Color::Rgb(40, 8, 12);
const FOE: Color = Color::Rgb(255, 80, 80);

/// One actor in the order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    /// `YOU`, `goon`, `grey ICE`.
    pub name: String,
    /// `18`, `4/6`; nothing for you.
    pub hp: String,
    pub acting: bool,
}

/// What the column reads this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnView {
    pub round: u32,
    pub chips: Vec<Chip>,
    /// Your pools while you act: (moves, moves max, ap, ap max) and whether the
    /// act half is on.
    pub yours: Option<(u8, u8, u8, u8, bool)>,
    /// What the acting enemy is up to: `moves 2 › fires`.
    pub theirs: Option<String>,
}

pub struct TurnOverlay<'a> {
    pub view: &'a TurnView,
}

#[allow(clippy::too_many_arguments)]
fn put(buf: &mut Buffer, area: Rect, x0: u16, y: u16, s: &str, fg: Color, bg: Color, bold: bool) {
    if y >= area.bottom() {
        return;
    }
    for (i, ch) in s.chars().enumerate() {
        let x = x0 + i as u16;
        if x >= area.right() {
            break;
        }
        let cell = &mut buf[(x, y)];
        cell.set_char(ch).set_fg(fg).set_bg(bg);
        cell.modifier.remove(Modifier::DIM);
        if bold {
            cell.modifier.insert(Modifier::BOLD);
        } else {
            cell.modifier.remove(Modifier::BOLD);
        }
    }
}

/// A row of the column, padded to its width.
fn row(s: &str) -> String {
    let n = s.chars().count();
    let mut out = s.to_string();
    for _ in n..WIDTH as usize {
        out.push(' ');
    }
    out.chars().take(WIDTH as usize).collect()
}

impl Overlay for TurnOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        _origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        if area.width < WIDTH {
            return;
        }
        let x0 = area.right() - WIDTH;
        let mut y = area.y;
        let v = self.view;
        put(
            buf,
            area,
            x0,
            y,
            &row(&format!(" TURN {} ", v.round)),
            DIM,
            BLACK,
            true,
        );
        y += 1;
        for chip in &v.chips {
            let mark = if chip.acting { "▶" } else { " " };
            let name: String = chip.name.chars().take(9).collect();
            let line = row(&format!(" {mark} {name:<9}{:>5} ", chip.hp));
            let yours = chip.name == "YOU";
            let (fg, bg) = match (chip.acting, yours) {
                (true, true) => (CYAN, YOURS),
                (true, false) => (FOE, THEIRS),
                (false, _) => (DIM, BLACK),
            };
            put(buf, area, x0, y, &line, fg, bg, chip.acting);
            y += 1;
            if !chip.acting {
                continue;
            }
            if yours && let Some((moves, moves_max, ap, ap_max, acting)) = v.yours {
                let bar: String = (0..moves_max)
                    .map(|i| if i < moves { '▰' } else { '▱' })
                    .collect();
                let dots: String = (0..ap_max)
                    .map(|i| if i < ap { '●' } else { '○' })
                    .collect();
                let (mm, am) = if acting { (" ", "◆") } else { ("◆", " ") };
                let move_row = row(&format!(" {mm} MOVE {bar:<4}  {moves}/{moves_max} "));
                let ap_row = row(&format!(" {am} AP   {dots:<4}  {ap}/{ap_max} "));
                let (mf, mb, af, ab) = if acting {
                    (DIM, YOURS, CYAN, LIT)
                } else {
                    (CYAN, LIT, DIM, YOURS)
                };
                put(buf, area, x0, y, &move_row, mf, mb, !acting);
                put(buf, area, x0, y + 1, &ap_row, af, ab, acting);
                y += 2;
            } else if let Some(doing) = &v.theirs {
                put(
                    buf,
                    area,
                    x0,
                    y,
                    &row(&format!("   {doing}")),
                    TEXT,
                    THEIRS,
                    false,
                );
                y += 1;
            }
        }
    }
}
