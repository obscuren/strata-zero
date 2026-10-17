//! The aim: the path you mean to walk, lit as far as your moves carry.

use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::ui::chrome::CYAN;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The cross of an aim no path reaches.
pub const NO_WAY: Color = Color::Rgb(255, 40, 40);

pub struct PathOverlay<'a> {
    /// The tiles after your own, in walking order, ending on the aim.
    pub path: &'a [Pos],
    /// How many of them your moves cover.
    pub lit: usize,
    /// Where the cursor points; drawn red when no path reaches it.
    pub aim: Pos,
    pub reachable: bool,
}

impl Overlay for PathOverlay<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        if !self.reachable {
            if let Some((sx, sy)) = tile_to_screen(area, origin, self.aim) {
                buf[(sx, sy)]
                    .set_char('×')
                    .set_style(Style::default().fg(NO_WAY).add_modifier(Modifier::BOLD));
            }
            return;
        }
        let last = self.path.len().saturating_sub(1);
        for (i, p) in self.path.iter().enumerate() {
            let Some((sx, sy)) = tile_to_screen(area, origin, *p) else {
                continue;
            };
            let lit = i < self.lit;
            let style = if lit {
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(CYAN).add_modifier(Modifier::DIM)
            };
            buf[(sx, sy)]
                .set_char(if i == last { '×' } else { '·' })
                .set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::MapRef;
    use citygen::{City, GenParams};

    fn draw(o: &PathOverlay) -> Buffer {
        let city = City::empty(GenParams::default());
        let map = MapRef::stratum_of(&city, citygen::StratumKind::Street);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        let style = StylePack::load().unwrap();
        o.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        buf
    }

    #[test]
    fn the_lit_prefix_is_bold_cyan_the_rest_dim_and_the_end_a_cross() {
        let path: Vec<Pos> = (1..=6).map(|x| Pos::new(x, 1)).collect();
        let buf = draw(&PathOverlay {
            path: &path,
            lit: 4,
            aim: Pos::new(6, 1),
            reachable: true,
        });
        for x in 1..=4 {
            let c = &buf[(x as u16, 1)];
            assert_eq!(c.symbol(), "·", "tile {x}");
            assert!(
                c.style().add_modifier.contains(Modifier::BOLD),
                "tile {x} lit"
            );
        }
        let c = &buf[(5, 1)];
        assert_eq!(c.symbol(), "·");
        assert!(
            c.style().add_modifier.contains(Modifier::DIM),
            "beyond the moves: dim"
        );
        let end = &buf[(6, 1)];
        assert_eq!(end.symbol(), "×");
        assert!(
            end.style().add_modifier.contains(Modifier::DIM),
            "an end out of reach is dim"
        );
        assert_eq!(
            buf[(0, 1)].symbol(),
            " ",
            "your own tile is not on the path"
        );
    }

    #[test]
    fn an_end_within_the_moves_is_lit_and_no_path_draws_a_red_cross() {
        let path: Vec<Pos> = (1..=3).map(|x| Pos::new(x, 1)).collect();
        let buf = draw(&PathOverlay {
            path: &path,
            lit: 4,
            aim: Pos::new(3, 1),
            reachable: true,
        });
        let end = &buf[(3, 1)];
        assert_eq!(end.symbol(), "×");
        assert!(end.style().add_modifier.contains(Modifier::BOLD));
        assert!(!end.style().add_modifier.contains(Modifier::DIM));
        let buf = draw(&PathOverlay {
            path: &[],
            lit: 4,
            aim: Pos::new(7, 2),
            reachable: false,
        });
        let c = &buf[(7, 2)];
        assert_eq!(c.symbol(), "×");
        assert_eq!(
            c.style().fg,
            Some(Color::Rgb(255, 40, 40)),
            "no way there: red"
        );
    }
}
