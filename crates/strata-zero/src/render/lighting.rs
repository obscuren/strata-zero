//! Light and visibility applied to cells.

use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, screen_to_tile};
use crate::render::{Cell, Rgb, Visibility};
use citygen::{Grid, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Dev: every visible tile goes grey by its light value, glyph kept, so a tuning
/// change can be seen against the map.
pub struct LightOverlay<'a> {
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for LightOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for sy in area.y..area.bottom() {
            for sx in area.x..area.right() {
                let p = screen_to_tile(area, origin, (sx, sy));
                if map.tile(p).is_none() {
                    continue;
                }
                if self
                    .vis
                    .is_some_and(|v| v.get(p) != Some(&Visibility::Visible))
                {
                    continue;
                }
                let l = map.light(p);
                let grey = Rgb(l, l, l);
                let fg = if l < 128 {
                    Rgb(230, 230, 230)
                } else {
                    Rgb(20, 20, 20)
                };
                buf[(sx, sy)]
                    .set_bg(grey.scale(0.6).to_color())
                    .set_fg(fg.to_color());
            }
        }
    }
}

/// The sky's cast on this map right now, as the tileset mixes it.
pub fn sky_of(map: &dyn TileMap) -> Option<(Rgb, f32)> {
    map.sky().map(|((r, g, b), w)| (Rgb(r, g, b), w))
}

/// Darkens a colour by the tile's light level; never below a quarter so the map stays legible.
pub fn lit(c: Rgb, light: u8) -> Rgb {
    c.scale(0.25 + 0.75 * (light as f32) / 255.0)
}

/// A colour under coloured light `l`: each channel scaled by its own light, never below
/// a floor, plus a little of the light itself. A foreground keeps more than a background.
pub fn lit_by(c: Rgb, l: Rgb, fg: bool) -> Rgb {
    let (floor, glow) = if fg { (0.30, 0.16) } else { (0.22, 0.10) };
    let ch = |c: u8, l: u8| {
        let l = l as f32;
        (c as f32 * (floor + (1.0 - floor) * l / 255.0) + l * glow)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgb(ch(c.0, l.0), ch(c.1, l.1), ch(c.2, l.2))
}

/// Remembered tiles are dim and static; unseen tiles are black.
pub fn apply_vis(cell: Cell, vis: Visibility) -> Cell {
    match vis {
        Visibility::Visible => cell,
        Visibility::Remembered => Cell {
            fg: cell.fg.scale(0.35),
            bg: cell.bg.scale(0.35),
            bold: false,
            dim: true,
            ..cell
        },
        Visibility::Unseen => Cell {
            ch: ' ',
            fg: Rgb(0, 0, 0),
            bg: Rgb(0, 0, 0),
            bold: false,
            dim: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Cell, Rgb};

    #[test]
    fn the_light_overlay_paints_grey_by_light_and_keeps_the_glyph() {
        use citygen::{GenParams, Generator, SizePreset, StratumKind};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        let s = crate::render::style::StylePack::load().unwrap();
        let city = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let map = crate::render::map::MapRef::stratum_of(&city, StratumKind::Street);
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        buf[(0, 0)].set_char('x');
        LightOverlay { vis: None }.draw(&map, citygen::Pos::new(10, 10), area, 0, &mut buf, &s);
        let l = map.light(citygen::Pos::new(10, 10));
        assert_eq!(buf[(0, 0)].bg, Rgb(l, l, l).scale(0.6).to_color());
        assert_eq!(buf[(0, 0)].symbol(), "x", "the glyph stays");
        // Unseen tiles are left alone.
        let unseen = citygen::Grid::filled(city.size, Visibility::Unseen);
        let mut dark = Buffer::empty(area);
        LightOverlay { vis: Some(&unseen) }.draw(
            &map,
            citygen::Pos::new(10, 10),
            area,
            0,
            &mut dark,
            &s,
        );
        assert_eq!(dark[(0, 0)].bg, Buffer::empty(area)[(0, 0)].bg);
    }

    #[test]
    fn lit_scales_between_quarter_and_full() {
        assert_eq!(lit(Rgb(200, 100, 40), 255), Rgb(200, 100, 40));
        assert_eq!(lit(Rgb(200, 100, 40), 0), Rgb(50, 25, 10));
        let c = Cell {
            ch: 'x',
            fg: Rgb(200, 200, 200),
            bg: Rgb(20, 20, 20),
            bold: true,
            dim: false,
        };
        let r = apply_vis(c, Visibility::Remembered);
        assert!(r.fg.0 < 100 && r.dim && !r.bold);
        assert_eq!(apply_vis(c, Visibility::Visible), c);
    }
}
