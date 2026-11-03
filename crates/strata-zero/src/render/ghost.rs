//! Signal ghost: where each awake piece of ICE steps next, marked on the map in its
//! colour. Everything in a construct is in view now, so the ghost reads intent.

use crate::render::map::MapRef;
use crate::render::netlook::ice_colour;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use citygen::{IceGrade, Pos};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

pub struct GhostOverlay {
    /// Each awake piece's next tile, with its grade.
    pub steps: Vec<(Pos, IceGrade)>,
}

impl Overlay for GhostOverlay {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for (p, grade) in &self.steps {
            if let Some(at) = tile_to_screen(area, origin, *p) {
                let cell = &mut buf[at];
                cell.set_char('·')
                    .set_fg(ice_colour(*grade, tick).to_color());
                cell.modifier.insert(Modifier::BOLD);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset, StratumKind};

    #[test]
    fn a_step_is_drawn_where_the_piece_goes() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let map = MapRef::stratum_of(&c, StratumKind::Net);
        let area = Rect::new(0, 0, 6, 4);
        let mut buf = Buffer::empty(area);
        GhostOverlay {
            steps: vec![(Pos::new(2, 1), IceGrade::Grey)],
        }
        .draw(
            &map,
            Pos::new(0, 0),
            area,
            0,
            &mut buf,
            &StylePack::load().unwrap(),
        );
        assert_eq!(buf[(2, 1)].symbol(), "·");
        assert_eq!(buf[(3, 1)].symbol(), " ");
    }
}
