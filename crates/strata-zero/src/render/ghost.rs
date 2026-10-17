//! Signal ghost: the awake ICE a netrunner feels through a construct's walls,
//! drawn where it stands whether or not it is in sight.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::MAGENTA;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

pub struct GhostOverlay {
    pub pieces: Vec<Pos>,
}

impl Overlay for GhostOverlay {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for p in &self.pieces {
            if let Some((sx, sy)) = tile_to_screen(area, origin, *p) {
                buf[(sx, sy)]
                    .set_char('▣')
                    .set_style(Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD));
            }
        }
    }
}
