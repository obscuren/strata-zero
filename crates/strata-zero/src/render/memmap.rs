//! The player's memory of a stratum: only what has been seen, scaled to fit.

use crate::render::Visibility;
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use citygen::{Grid, Pos, Rect as TileRect};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

pub struct MemoryMap<'a> {
    pub map: &'a MapRef<'a>,
    pub vis: &'a Grid<Visibility>,
    pub player: Option<Pos>,
    pub style: &'a StylePack,
}

impl Widget for MemoryMap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let size = self.map.size();
        let scale = (size.w as f32 / area.width as f32)
            .max(size.h as f32 / area.height as f32)
            .max(1.0)
            .ceil() as i32;
        let bg0 = self.style.background;
        for cy in 0..area.height {
            for cx in 0..area.width {
                let block = TileRect::new(cx as i32 * scale, cy as i32 * scale, scale, scale);
                let (mut total, mut seen, mut visible, mut walk) = (0usize, 0usize, 0usize, 0usize);
                let mut theme = None;
                for p in block.iter() {
                    let Some(t) = self.map.tile(p) else { continue };
                    total += 1;
                    match self.vis.get(p).copied().unwrap_or(Visibility::Unseen) {
                        Visibility::Unseen => continue,
                        Visibility::Remembered => seen += 1,
                        Visibility::Visible => {
                            seen += 1;
                            visible += 1;
                        }
                    }
                    if t.is_walkable() {
                        walk += 1;
                    }
                    if theme.is_none() {
                        theme = self.map.theme(p);
                    }
                }
                let cell = &mut buf[(area.x + cx, area.y + cy)];
                if total == 0 || seen == 0 {
                    cell.set_char(' ').set_bg(bg0.to_color());
                    continue;
                }
                let accent = self.style.theme_or_neutral(theme).accent;
                let strength = if visible > 0 { 0.7 } else { 0.35 };
                let f = strength * (0.3 + 0.7 * walk as f32 / seen as f32);
                cell.set_char(' ').set_bg(accent.scale(f).to_color());
            }
        }
        if let Some(p) = self.player {
            let (cx, cy) = (p.x / scale, p.y / scale);
            if cx >= 0 && cy >= 0 && cx < area.width as i32 && cy < area.height as i32 {
                buf[(area.x + cx as u16, area.y + cy as u16)]
                    .set_char('@')
                    .set_fg(Color::Rgb(255, 255, 255));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::Player;
    use citygen::{GenParams, Generator, SizePreset, StratumKind};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn memory_map_draws_only_what_was_seen() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let s = StylePack::load().unwrap();
        let p = Player::spawn(&c, 1);
        let mut t = Terminal::new(TestBackend::new(60, 30)).unwrap();
        t.draw(|f| {
            MemoryMap {
                map: &MapRef::stratum_of(&c, StratumKind::Street),
                vis: p.vis(),
                player: Some(p.pos),
                style: &s,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        let buf = t.backend().buffer();
        let lit = (0..30u16)
            .flat_map(|y| (0..60u16).map(move |x| (x, y)))
            .filter(|(x, y)| buf[(*x, *y)].bg != s.background.to_color())
            .count();
        assert!(lit > 0 && lit < 60 * 30 / 2, "lit cells {lit}");
        assert!((0..30u16).any(|y| (0..60u16).any(|x| buf[(x, y)].symbol() == "@")));
        // Tiny areas never panic.
        let mut t = Terminal::new(TestBackend::new(1, 1)).unwrap();
        t.draw(|f| {
            MemoryMap {
                map: &MapRef::stratum_of(&c, StratumKind::Street),
                vis: p.vis(),
                player: None,
                style: &s,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
    }
}
