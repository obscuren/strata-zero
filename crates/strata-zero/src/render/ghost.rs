//! Signal ghost: the awake ICE a netrunner feels through a construct's walls,
//! drawn where it stands whether or not it is in sight.

use crate::render::Visibility;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::MAGENTA;
use citygen::{Grid, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

pub struct GhostOverlay<'a> {
    pub pieces: Vec<Pos>,
    /// The player's sight: a piece in plain view keeps the map's own glyph and
    /// its grade's colour; the ghost draws only what the map cannot.
    pub vis: Option<&'a Grid<Visibility>>,
}

/// The felt pieces to draw: every one not currently visible.
pub fn ghost_cells(pieces: &[Pos], vis: Option<&Grid<Visibility>>) -> Vec<Pos> {
    pieces
        .iter()
        .copied()
        .filter(|p| !vis.is_some_and(|v| v.get(*p) == Some(&Visibility::Visible)))
        .collect()
}

impl Overlay for GhostOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for p in ghost_cells(&self.pieces, self.vis) {
            if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                buf[(sx, sy)]
                    .set_char('▣')
                    .set_style(Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Visibility;
    use citygen::{Grid, Size};

    #[test]
    fn a_felt_piece_is_drawn_only_where_the_map_cannot_see_it() {
        let mut vis = Grid::filled(Size { w: 6, h: 4 }, Visibility::Unseen);
        vis[Pos::new(1, 1)] = Visibility::Visible;
        vis[Pos::new(2, 1)] = Visibility::Remembered;
        let pieces = vec![Pos::new(1, 1), Pos::new(2, 1), Pos::new(4, 2)];
        assert_eq!(
            ghost_cells(&pieces, Some(&vis)),
            vec![Pos::new(2, 1), Pos::new(4, 2)],
            "the visible piece keeps the map's own glyph and grade colour"
        );
        assert_eq!(
            ghost_cells(&pieces, None),
            pieces,
            "no sight grid: draw them all"
        );
    }
}
