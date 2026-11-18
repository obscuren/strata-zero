//! The bar over a container while a lock is forced.

use crate::render::Visibility;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::FAINTER;
use citygen::{Grid, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Cells in the bar.
pub const BAR_CELLS: i32 = 5;
const FILLED: Color = Color::Rgb(0xff, 0xb0, 0x00);

/// Five cells on the row above `at` (below when that row is off the view), centred, filled
/// `frac` of the way. Hidden when the container's tile isn't in sight.
pub struct ForceBar<'a> {
    pub at: Pos,
    pub frac: f32,
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for ForceBar<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        if let Some(v) = self.vis
            && v.get(self.at).copied() != Some(Visibility::Visible)
        {
            return;
        }
        let Some((sx, sy)) = tile_to_screen(area, origin, self.at) else {
            return;
        };
        let row = if sy > area.y {
            sy - 1
        } else if sy + 1 < area.bottom() {
            sy + 1
        } else {
            return;
        };
        let filled = (self.frac.clamp(0.0, 1.0) * BAR_CELLS as f32).round() as i32;
        for i in 0..BAR_CELLS {
            let x = sx as i32 - BAR_CELLS / 2 + i;
            if x < area.x as i32 || x >= area.right() as i32 {
                continue;
            }
            let cell = &mut buf[(x as u16, row)];
            if i < filled {
                cell.set_char('█').set_fg(FILLED);
            } else {
                cell.set_char('░').set_fg(FAINTER);
            }
        }
    }
}
