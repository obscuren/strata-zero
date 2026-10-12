//! A downscaled district map with the viewport rectangle and the player.

use crate::render::Rgb;
use crate::render::style::StylePack;
use citygen::{City, Pos, Rect as TileRect, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

pub struct Minimap<'a> {
    pub city: &'a City,
    pub stratum: StratumKind,
    /// The main viewport, in tiles.
    pub view: TileRect,
    pub player: Option<Pos>,
    pub style: &'a StylePack,
}

impl Widget for Minimap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let Some(st) = self.city.stratum(self.stratum) else {
            return;
        };
        let size = self.city.size;
        let scale = (size.w as f32 / area.width as f32)
            .max(size.h as f32 / area.height as f32)
            .max(1.0)
            .ceil() as i32;
        let to_cell = |p: Pos| -> Option<(u16, u16)> {
            let cx = p.x / scale;
            let cy = p.y / scale;
            if cx >= 0 && cy >= 0 && cx < area.width as i32 && cy < area.height as i32 {
                Some((area.x + cx as u16, area.y + cy as u16))
            } else {
                None
            }
        };
        for cy in 0..area.height {
            for cx in 0..area.width {
                let block = TileRect::new(cx as i32 * scale, cy as i32 * scale, scale, scale);
                let mut walk = 0usize;
                let mut total = 0usize;
                let mut theme = None;
                let mut water = 0usize;
                for p in block.iter() {
                    let Some(t) = st.tiles.get(p) else { continue };
                    total += 1;
                    if t.is_walkable() {
                        walk += 1;
                    }
                    if t.terrain == citygen::Terrain::Water {
                        water += 1;
                    }
                    if theme.is_none() {
                        theme = t.district.map(|d| self.city.district(d).theme);
                    }
                }
                let cell = &mut buf[(area.x + cx, area.y + cy)];
                if total == 0 {
                    cell.set_char(' ').set_bg(self.style.background.to_color());
                    continue;
                }
                let bg = if water * 2 > total {
                    Rgb(10, 24, 50)
                } else {
                    let accent = self.style.theme_or_neutral(theme).accent;
                    let f = 0.15 + 0.45 * (walk as f32 / total as f32);
                    accent.scale(f)
                };
                cell.set_char(' ').set_bg(bg.to_color());
            }
        }
        // Viewport rectangle.
        let v = self.view;
        let corners = [
            (Pos::new(v.x, v.y), '┌'),
            (Pos::new(v.right() - 1, v.y), '┐'),
            (Pos::new(v.x, v.bottom() - 1), '└'),
            (Pos::new(v.right() - 1, v.bottom() - 1), '┘'),
        ];
        let frame = Color::Rgb(230, 230, 240);
        for x in v.x..v.right() {
            for y in [v.y, v.bottom() - 1] {
                if let Some((sx, sy)) = to_cell(Pos::new(x, y)) {
                    buf[(sx, sy)].set_char('─').set_fg(frame);
                }
            }
        }
        for y in v.y..v.bottom() {
            for x in [v.x, v.right() - 1] {
                if let Some((sx, sy)) = to_cell(Pos::new(x, y)) {
                    buf[(sx, sy)].set_char('│').set_fg(frame);
                }
            }
        }
        for (p, ch) in corners {
            if let Some((sx, sy)) = to_cell(p) {
                buf[(sx, sy)].set_char(ch).set_fg(frame);
            }
        }
        if let Some(p) = self.player
            && let Some((sx, sy)) = to_cell(p)
        {
            buf[(sx, sy)]
                .set_char('@')
                .set_fg(Color::Rgb(255, 255, 255));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use citygen::{GenParams, Generator, Pos, Rect, SizePreset, StratumKind};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    #[test]
    fn minimap_fits_any_area_and_marks_player() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let s = StylePack::load().unwrap();
        for (w, h) in [(1u16, 1u16), (24, 12), (80, 40)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| {
                Minimap {
                    city: &c,
                    stratum: StratumKind::Street,
                    view: Rect::new(10, 10, 30, 15),
                    player: Some(Pos::new(20, 20)),
                    style: &s,
                }
                .render(f.area(), f.buffer_mut())
            })
            .unwrap();
            if (w, h) == (80, 40) {
                let b = t.backend().buffer();
                let found = (0..40).any(|y| (0..80).any(|x| b[(x, y)].symbol() == "@"));
                assert!(found);
            }
        }
    }
}
