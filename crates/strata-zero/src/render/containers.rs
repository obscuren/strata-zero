//! What is drawn over a container: its open, empty and on-the-Net looks, and the bar while a
//! lock is forced.

use crate::containers::ContainerLook;
use crate::render::Visibility;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::tileset::{
    CONTAINER_EMPTY_BG, CONTAINER_EMPTY_FG, CONTAINER_NET, container_body, container_detail,
    container_open_shape,
};
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::FAINTER;
use citygen::tile::ContainerKind;
use citygen::{Grid, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Cells in the bar.
pub const BAR_CELLS: i32 = 5;
const FILLED: Color = Color::Rgb(0xff, 0xb0, 0x00);

/// Open, emptied and on-the-Net containers: an open one its shape in the detail colour on
/// the body, an empty one its shape greyed, one on the Net its detail in cyan. Hidden on
/// unseen tiles, dimmed on remembered ones; with no visibility grid every one shows.
pub struct ContainerLooks<'a> {
    pub looks: &'a [(Pos, ContainerKind, ContainerLook)],
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for ContainerLooks<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for &(pos, kind, look) in self.looks {
            let Some((sx, sy)) = tile_to_screen(area, origin, pos) else {
                continue;
            };
            let seen = match self.vis {
                None => Visibility::Visible,
                Some(v) => v.get(pos).copied().unwrap_or(Visibility::Unseen),
            };
            let strength = match seen {
                Visibility::Visible => 1.0,
                Visibility::Remembered => 0.45,
                Visibility::Unseen => continue,
            };
            let (detail, detail_fg) = container_detail(kind);
            let (glyph, fg, bg) = match look {
                ContainerLook::Open => {
                    (container_open_shape(kind), detail_fg, container_body(kind))
                }
                ContainerLook::Empty => (
                    container_open_shape(kind),
                    CONTAINER_EMPTY_FG,
                    CONTAINER_EMPTY_BG,
                ),
                ContainerLook::Net => (detail, CONTAINER_NET, container_body(kind)),
            };
            buf[(sx, sy)]
                .set_char(glyph)
                .set_fg(fg.scale(strength).to_color())
                .set_bg(bg.scale(strength).to_color());
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, Size, SizePreset, StratumKind};

    #[test]
    fn each_state_draws_as_the_key_says() {
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let map = MapRef::stratum_of(&city, StratumKind::Street);
        let looks = [
            (Pos::new(0, 0), ContainerKind::Safe, ContainerLook::Open),
            (Pos::new(1, 0), ContainerKind::Locker, ContainerLook::Empty),
            (Pos::new(2, 0), ContainerKind::Safe, ContainerLook::Net),
            (Pos::new(3, 0), ContainerKind::Crate, ContainerLook::Open),
            (Pos::new(4, 0), ContainerKind::Crate, ContainerLook::Open),
        ];
        let mut vis = Grid::filled(Size { w: 20, h: 10 }, Visibility::Visible);
        vis.set(Pos::new(3, 0), Visibility::Remembered);
        vis.set(Pos::new(4, 0), Visibility::Unseen);
        let area = Rect::new(0, 0, 5, 1);
        let style = StylePack::load().unwrap();
        let mut buf = Buffer::empty(area);
        let overlay = ContainerLooks {
            looks: &looks,
            vis: Some(&vis),
        };
        overlay.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        let rgb = |r, g, b| Color::Rgb(r, g, b);
        assert_eq!(buf[(0, 0)].symbol(), "○");
        assert_eq!(buf[(0, 0)].bg, rgb(0x2b, 0x2f, 0x36));
        assert_eq!(buf[(1, 0)].symbol(), "▯");
        assert_eq!(buf[(1, 0)].fg, rgb(0x5b, 0x6b, 0x7d));
        assert_eq!(buf[(1, 0)].bg, rgb(0x20, 0x26, 0x2e));
        assert_eq!(buf[(2, 0)].symbol(), "◉");
        assert_eq!(buf[(2, 0)].fg, rgb(0x00, 0xe6, 0xd2));
        assert_eq!(buf[(3, 0)].symbol(), "□");
        assert_ne!(
            buf[(3, 0)].bg,
            container_body(ContainerKind::Crate).to_color(),
            "remembered: dimmed"
        );
        assert_eq!(buf[(4, 0)].symbol(), " ", "unseen: untouched");
    }
}
